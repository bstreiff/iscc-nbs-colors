// Validation tool for iscc-nbs.xml.
//
// SPDX-License-Identifier: MIT

extern crate is_sorted;

use is_sorted::IsSorted;

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::process::Command;

use geo::extremes::Extremes;
use geo::Centroid;
use geo_clipper::Clipper;
use geo_types::{Coordinate, LineString, Polygon};

struct ColorName {
    name: String,
    abbr: String,
}

struct ColorBlock {
    color_id: u32,
    hues: Range<usize>,
    chromas: Range<usize>,
    values: Range<usize>,
}

fn add_name_to_map(map: &mut HashMap<u32, ColorName>, node: roxmltree::Node) {
    let color_id: u32 = node.attribute("color").unwrap().parse::<u32>().unwrap();
    let color_name = node.attribute("name").unwrap().to_string();
    let color_abbr = node.attribute("abbr").unwrap().to_string();

    if map.contains_key(&color_id) {
        println!(
            "Error: Conflicting color ids for {}: {} and {}.",
            color_id,
            map.get(&color_id).unwrap().name,
            color_name
        );
        std::process::exit(1);
    }

    map.insert(
        color_id,
        ColorName {
            name: color_name,
            abbr: color_abbr,
        },
    );
}

fn validate_name_map(map: &HashMap<u32, ColorName>) {
    let mut max_color_id: u32 = 0;

    for (color_id, name_entry) in map.iter() {
        if color_id > &max_color_id {
            max_color_id = *color_id
        }

        // ensure that this name and abbr are unused elsewhere
        for (color2_id, name2_entry) in map.iter() {
            if color_id == color2_id {
                continue; // but don't match ourselves!
            }
            if name_entry.name == name2_entry.name {
                println!(
                    "Error: Duplicate name '{}' used for both id {} and {}.",
                    name_entry.name, color_id, color2_id
                );
                std::process::exit(1);
            }
            if name_entry.abbr == name2_entry.abbr {
                println!(
                    "Error: Duplicate abbr '{}' used for both id {} and {}.",
                    name_entry.abbr, color_id, color2_id
                );
                std::process::exit(1);
            }
        }
    }

    // also ensure that all ids from 1..max_color_id are present
    for id in 1..max_color_id {
        if !map.contains_key(&id) {
            println!("Error: missing color id {} in 1..{}.", id, max_color_id);
            std::process::exit(1);
        }
    }
}

fn validate_names(doc: &roxmltree::Document) -> HashMap<u32, ColorName> {
    let names = doc.descendants().find(|n| n.has_tag_name("names")).unwrap();

    let mut level1_names = HashMap::new();
    let mut level2_names = HashMap::new();
    let mut level3_names = HashMap::new();

    for level1 in names.children().filter(|n| n.is_element()) {
        add_name_to_map(&mut level1_names, level1);
        for level2 in level1.children().filter(|n| n.is_element()) {
            add_name_to_map(&mut level2_names, level2);
            for level3 in level2.children().filter(|n| n.is_element()) {
                add_name_to_map(&mut level3_names, level3);
            }
        }
    }

    validate_name_map(&level1_names);
    validate_name_map(&level2_names);
    validate_name_map(&level3_names);

    return level3_names;
}

fn get_hues(doc: &roxmltree::Document) -> Vec<String> {
    let mut amounts: Vec<String> = Vec::new();

    let values = doc.descendants().find(|n| n.has_tag_name("hues")).unwrap();

    for amount_elem in values.children().filter(|n| n.is_element()) {
        amounts.push(amount_elem.attribute("id").unwrap().to_string());
    }

    return amounts;
}

fn get_amount_list(tag_name: &str, doc: &roxmltree::Document) -> Vec<String> {
    let mut amounts: Vec<String> = Vec::new();

    let values = doc
        .descendants()
        .find(|n| n.has_tag_name(tag_name))
        .unwrap();

    for amount_elem in values.children().filter(|n| n.is_element()) {
        amounts.push(amount_elem.text().unwrap().to_string());
    }

    // We actually want to keep these values as strings for index lookup, but
    // also we do want to verify that these are floating-point values in sorted
    // order.

    let mut amounts_f32 = amounts
        .clone()
        .into_iter()
        .map(|x| x.parse::<f32>().unwrap());
    if !IsSorted::is_sorted(&mut amounts_f32) {
        println!("Error: {} array is not in sorted order.", tag_name);
        std::process::exit(1);
    }

    return amounts;
}

fn get_chromas(doc: &roxmltree::Document) -> Vec<String> {
    return get_amount_list("chromas", &doc);
}

fn get_values(doc: &roxmltree::Document) -> Vec<String> {
    return get_amount_list("values", &doc);
}

fn validate_blocks(
    doc: &roxmltree::Document,
    hues: &Vec<String>,
    chromas: &Vec<String>,
    values: &Vec<String>,
) -> Vec<ColorBlock> {
    // The lookup table is logically a three-dimensional array, but initializing a
    // vector of vectors of vectors is Actually Kind Of A Pain?
    //
    // We remove one from chroma and values length because of the INF at the end.
    let mut lookup_table: Vec<u32> =
        Vec::with_capacity(hues.len() * (chromas.len() - 1) * (values.len() - 1));
    let mut blocks: Vec<ColorBlock> = Vec::new();

    lookup_table.resize(hues.len() * (chromas.len() - 1) * (values.len() - 1), 0);
    let index = |h: usize, c: usize, v: usize| -> Option<usize> {
        if h > hues.len() {
            return None;
        }
        if c > (chromas.len() - 1) {
            return None;
        }
        if v > (values.len() - 1) {
            return None;
        }
        return Some((h * (chromas.len() - 1) * (values.len() - 1)) + (c * (values.len() - 1)) + v);
    };

    let ranges = doc
        .descendants()
        .find(|n| n.has_tag_name("ranges"))
        .unwrap();

    for huerange in ranges.children().filter(|n| n.is_element()) {
        let hue_begin_index = hues
            .iter()
            .position(|x| x == huerange.attribute("begin").unwrap())
            .unwrap();
        let hue_end_index = hues
            .iter()
            .position(|x| x == huerange.attribute("end").unwrap())
            .unwrap();

        // hues will wrap around; ensure that begin < logical_end, and then
        // when using the hue index later we'll mod it by length
        let hue_logical_end_index;
        if hue_end_index < hue_begin_index {
            hue_logical_end_index = hue_end_index + hues.len();
        } else {
            hue_logical_end_index = hue_end_index;
        }

        for range in huerange.children().filter(|n| n.is_element()) {
            let color_id = range.attribute("color").unwrap().parse::<u32>().unwrap();
            let chroma_begin_index = chromas
                .iter()
                .position(|x| x == range.attribute("chroma-begin").unwrap())
                .unwrap();
            let chroma_end_index = chromas
                .iter()
                .position(|x| x == range.attribute("chroma-end").unwrap())
                .unwrap();
            let value_begin_index = values
                .iter()
                .position(|x| x == range.attribute("value-begin").unwrap())
                .unwrap();
            let value_end_index = values
                .iter()
                .position(|x| x == range.attribute("value-end").unwrap())
                .unwrap();

            for h in hue_begin_index..hue_logical_end_index {
                let h = h % hues.len();

                for c in chroma_begin_index..chroma_end_index {
                    for v in value_begin_index..value_end_index {
                        let idx = index(h, c, v).unwrap();

                        if lookup_table[idx] != 0 {
                            println!(
                                "Error: Trying to place color {} over {} at h={} c={} v={}",
                                color_id, lookup_table[idx], hues[h], chromas[c], values[v]
                            );
                            std::process::exit(1);
                        }

                        lookup_table[idx] = color_id;
                    }
                }
            }

            blocks.push(ColorBlock {
                color_id: color_id,
                hues: Range {
                    start: hue_begin_index,
                    end: hue_end_index,
                },
                chromas: Range {
                    start: chroma_begin_index,
                    end: chroma_end_index,
                },
                values: Range {
                    start: value_begin_index,
                    end: value_end_index,
                },
            })
        }
    }

    // now validate that all slots have been filled
    for h in 0..hues.len() {
        for c in 0..chromas.len() - 1 {
            for v in 0..values.len() - 1 {
                let idx = index(h, c, v).unwrap();

                if lookup_table[idx] == 0 {
                    println!(
                        "Error: No color placed at h={} c={} v={}",
                        hues[h], chromas[c], values[v]
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    return blocks;
}

fn deinfinite(x: String) -> String {
    if x == "INF" {
        "9999".to_string()
    } else {
        x
    }
}

fn linebreak(x: String) -> String {
    return x.replace(" ", "\\n");
}

fn generate_gnuplot(
    blocks: &Vec<ColorBlock>,
    hues: &Vec<String>,
    chromas: &Vec<String>,
    values: &Vec<String>,
    names: &HashMap<u32, ColorName>,
) {
    for h in 0..hues.len() {
        let hue_blocks = blocks.iter().filter(|x| h == x.hues.start);

        let basename = format!(
            "doc/page{}-{}_hues_{}-{}",
            16 + (h / 2),
            h % 2,
            hues[h],
            hues[(h + 1) % hues.len()]
        );
        let mut file = File::create(format!("{}.gnu", basename)).unwrap();

        writeln!(&mut file, "set encoding utf8").unwrap();
        writeln!(&mut file, "set xrange [ 0.0 : 16.9 ]").unwrap();
        writeln!(&mut file, "set yrange [ 0.0 : 10.4 ]").unwrap();
        writeln!(&mut file, "set grid xtics ytics").unwrap();
        writeln!(&mut file, "unset key").unwrap();
        writeln!(&mut file, "set border 3").unwrap();
        writeln!(&mut file, "set xlabel \"Munsell Chroma\"").unwrap();
        writeln!(&mut file, "set ylabel \"Munsell Value\"").unwrap();
        writeln!(
            &mut file,
            "set title \"{}-{}\" offset graph 0.45,0",
            hues[h],
            hues[(h + 1) % hues.len()]
        )
        .unwrap();

        writeln!(&mut file, "set style fill empty").unwrap();
        writeln!(&mut file, "set style line 1 default").unwrap();

        let mut has_0p7 = false;
        let mut has_1p2 = false;

        let mut regions: HashMap<u32, Polygon> = HashMap::new();

        for block in hue_blocks {
            let x1 = chromas[block.chromas.start].clone();
            let x2 = deinfinite(chromas[block.chromas.end].clone());
            let y1 = values[block.values.start].clone();
            let y2 = deinfinite(values[block.values.end].clone());

            let x1f: f64 = x1.parse().unwrap();
            let x2f: f64 = x2.parse::<f64>().unwrap().min(17.0);
            let y1f: f64 = y1.parse().unwrap();
            let y2f: f64 = y2.parse::<f64>().unwrap().min(10.5);

            if x1 == "0.7" || x2 == "0.7" {
                has_0p7 = true;
            }

            if x1 == "1.2" || x2 == "1.2" {
                has_1p2 = true;
            }

            let area = Polygon::new(
                LineString(vec![
                    Coordinate { x: x1f, y: y1f },
                    Coordinate { x: x1f, y: y2f },
                    Coordinate { x: x2f, y: y2f },
                    Coordinate { x: x2f, y: y1f },
                ]),
                vec![],
            );
            if regions.contains_key(&block.color_id) {
                let union = regions.get(&block.color_id).unwrap().union(&area, 10.0);
                regions.insert(block.color_id, union.into_iter().next().unwrap());
            } else {
                regions.insert(block.color_id, area);
            }
        }

        for (id, region) in regions.iter() {
            writeln!(&mut file, "").unwrap();
            writeln!(
                &mut file,
                "set object {} polygon from {} default",
                id + 1,
                region
                    .exterior()
                    .points()
                    .map(|v| format!("{},{}", v.x(), v.y()))
                    .collect::<Vec<String>>()
                    .join(" to ")
            )
            .unwrap();

            let extremes = region.extremes().unwrap();
            let poly_min = Coordinate {
                x: extremes.x_min.coord.x,
                y: extremes.y_min.coord.y,
            };
            let poly_max = Coordinate {
                x: extremes.x_max.coord.x,
                y: extremes.y_max.coord.y,
            };

            let label_pos = region.centroid().unwrap();
            let (label_x, label_y) = (label_pos.x(), label_pos.y());

            let is_vertical = (poly_max.y - poly_min.y) * 2.0 > (poly_max.x - poly_min.x);
            let rotate = if is_vertical {
                "rotate by 90"
            } else {
                "norotate"
            };
            let orig_name = names[&id].name.clone();

            // gnuplot doesn't actually know about the bounding boxes in which we want to
            // put these labels, which means it also doesn't know how to word-wrap them.
            // Ideally I'd be able to compute this given the font and the string, but that's
            // difficult, so just come up with a cheap-hopefully-good-enough-for-this-data
            // approach.
            let linebreak_thresh = (orig_name.len() as f64) * 0.2 + 0.5;
            let should_linebreak = !is_vertical && (poly_max.x - poly_min.x) < linebreak_thresh;
            let label_name = if should_linebreak {
                linebreak(orig_name)
            } else {
                orig_name
            };

            // should this be done with the "labels" plot style instead?
            writeln!(
                &mut file,
                "set label {} \"{{/:Bold {}}}: {}\" at first {},{} center {}",
                id + 1,
                id,
                label_name,
                label_x,
                label_y,
                rotate
            )
            .unwrap();
        }

        writeln!(
            &mut file,
            "set xtics border nomirror out scale 2.0 font 'Verdana,8'"
        )
        .unwrap();
        writeln!(&mut file, "set xtics 0, 2.0").unwrap();
        writeln!(&mut file, "set xtics add (1.0)").unwrap();
        if has_0p7 {
            writeln!(&mut file, "set xtics add (\"0.7\" 0.7 1)").unwrap();
            writeln!(
                &mut file,
                "set label 1000 \"0.7\" at first 0.65,-0.25 center font \"Verdana,6\""
            )
            .unwrap();
        }
        if has_1p2 {
            writeln!(&mut file, "set xtics add (\"1.2\" 1.2 1)").unwrap();
            writeln!(
                &mut file,
                "set label 1001 \"1.2\" at first 1.25,-0.25 center font \"Verdana,6\""
            )
            .unwrap();
        }

        writeln!(&mut file, "set mxtics 2").unwrap();
        writeln!(
            &mut file,
            "set ytics border nomirror out scale 2.0 font 'Verdana,8'"
        )
        .unwrap();
        writeln!(&mut file, "set ytics 0, 1.0").unwrap();
        writeln!(&mut file, "set mytics 2").unwrap();

        writeln!(
            &mut file,
            "set terminal pngcairo size 600,800 enhanced font 'Verdana,7'"
        )
        .unwrap();
        writeln!(&mut file, "set output '{}.png'", basename).unwrap();

        // we need to plot _something_; can't just have polygons
        writeln!(&mut file, "plot x+9999").unwrap();

        // close and flush the file
        drop(file);

        Command::new("gnuplot")
            .arg(format!("{}.gnu", basename))
            .status()
            .expect("failed to execute gnuplot");
    }
}

fn main() {
    let text = std::fs::read_to_string("iscc-nbs.xml").unwrap();

    let opt = roxmltree::ParsingOptions { allow_dtd: true };

    let doc = match roxmltree::Document::parse_with_options(&text, opt) {
        Ok(v) => v,
        Err(e) => {
            println!("Error: {}.", e);
            std::process::exit(1);
        }
    };

    let level3_names = validate_names(&doc);

    let hues = get_hues(&doc);
    let chromas = get_chromas(&doc);
    let values = get_values(&doc);

    let blocks = validate_blocks(&doc, &hues, &chromas, &values);

    generate_gnuplot(&blocks, &hues, &chromas, &values, &level3_names);
}
