// Validation tool for iscc-nbs.xml.
//
// SPDX-License-Identifier: MIT

extern crate is_sorted;

use is_sorted::IsSorted;

use std::collections::HashMap;

struct ColorName {
    name: String,
    abbr: String,
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

fn validate_names(doc: &roxmltree::Document) {
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
) {
    // The lookup table is logically a three-dimensional array, but initializing a
    // vector of vectors of vectors is Actually Kind Of A Pain?
    //
    // We remove one from chroma and values length because of the INF at the end.
    let mut lookup_table: Vec<u32> =
        Vec::with_capacity(hues.len() * (chromas.len() - 1) * (values.len() - 1));
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

    validate_names(&doc);

    let hues = get_hues(&doc);
    let chromas = get_chromas(&doc);
    let values = get_values(&doc);

    validate_blocks(&doc, &hues, &chromas, &values);
}
