import xml.etree.ElementTree as ET
from munsell import parse_munsell_colour, MUNSELL_COLOUR_FORMAT
from munsell import hue_to_hue_angle, hue_angle_to_hue
from munsell import munsell_specification_to_munsell_colour
import numpy as np
import math


def _hue_code_to_point(hue, hue_code):
    # put this hue and hue code combo on a 100-point scale, with 5Y at 90 degrees
    return (170.0 - ((hue_code)*10.0 - hue)) % 100.0


def _point_to_hue_and_code(point):
    point = -point + 170.0
    hue = 10.0 - (point % 10.0)
    hue_code = ((point - (point % 10.0)) / 10.0) % 10.0 + 1.0
    return hue, hue_code


def munsell_color_str_to_hvc(str):
    hue, value, chroma, hue_code = parse_munsell_colour(str)
    return (_hue_code_to_point(hue, hue_code), value, chroma)


def _get_hue_point(hue_str):
    full_str = MUNSELL_COLOUR_FORMAT.format(hue_str, 0.0, 0.0)
    hue, _, _ = munsell_color_str_to_hvc(full_str)
    return hue


def _is_between_points(hue, a, b):
    # normalize angles
    a = (100.0 + a % 100.0) % 100.0
    b = (100.0 + b % 100.0) % 100.0
    hue = (100.0 + hue % 100.0) % 100.0

    if (a < b):
        return a <= hue and hue <= b
    else:
        return a <= hue or hue <= b


def _degree_average(f1, f2):
    c1 = np.cos(np.deg2rad(f1))
    c2 = np.cos(np.deg2rad(f2))
    s1 = np.sin(np.deg2rad(f1))
    s2 = np.sin(np.deg2rad(f2))
    cavg = (c1 + c2) / 2.0
    savg = (s1 + s2) / 2.0

    return np.rad2deg(math.atan2(savg, cavg))


def _degree_diff(f1, f2):
    c = np.cos(np.deg2rad(f1) - np.deg2rad(f2))
    s = np.sin(np.deg2rad(f1) - np.deg2rad(f2))

    return np.rad2deg(np.fabs(math.atan2(s, c)))


class NamedColor(object):
    def __init__(self, id, name, abbr):
        self.id = id
        self.name = name
        self.abbr = abbr

    def __repr__(self):
        return f'NamedColor({self.id}, "{self.name}", "{self.abbr}")'


class ColorMatch(object):
    def __init__(self, id, hue_begin, hue_end, value_begin, value_end, chroma_begin, chroma_end):
        self.id = id
        self.hue_begin = hue_begin
        self.hue_end = hue_end
        self.value_begin = value_begin
        self.value_end = value_end
        self.chroma_begin = chroma_begin
        self.chroma_end = chroma_end

    def __repr__(self):
        return f'ColorMatch({self.id}, {self.hue_begin}, {self.hue_end}, {self.value_begin}, {self.value_end}, {self.chroma_begin}, {self.chroma_end})'

    def copy(self):
        return ColorMatch(self.id, self.hue_begin, self.hue_end, self.value_begin, self.value_end, self.chroma_begin, self.chroma_end)


class CentroidColorAccumulator(object):
    def __init__(self):
        self.value = 0
        self.chroma = 0
        self.hue_x = 0
        self.hue_y = 0
        self.volume = 0

    def append_block(self, block):
        hue_begin_deg = block.hue_begin * (360.0 / 100.0)
        hue_end_deg = block.hue_end * (360.0 / 100.0)
        chroma_begin = block.chroma_begin
        chroma_end = block.chroma_end
        value_begin = block.value_begin
        value_end = block.value_end

        if chroma_end > 16.0:
            chroma_end = 16.0
        if value_end > 10.0:
            value_end = 10.0

        hue_delta_deg = _degree_diff(hue_begin_deg, hue_end_deg)

        area_outer = chroma_end * chroma_end * (hue_delta_deg / 360.0)
        area_inner = chroma_begin * chroma_begin * (hue_delta_deg / 360.0)
        area = area_outer - area_inner
        volume = area * (value_end - value_begin)

        center_chroma = (chroma_begin + chroma_end) / 2.0
        center_value = (value_begin + value_end) / 2.0
        center_hue = _degree_average(hue_begin_deg, hue_end_deg)
        center_huex = math.cos(np.deg2rad(center_hue))
        center_huey = math.sin(np.deg2rad(center_hue))

        self.value += (center_value * volume)
        self.chroma += (center_chroma * volume)
        self.hue_x += (center_huex * volume)
        self.hue_y += (center_huey * volume)
        self.volume += volume

    def get_centroid(self):
        angle_degrees = np.rad2deg(math.atan2((self.hue_y / self.volume), (self.hue_x / self.volume)))
        hue, hue_code = _point_to_hue_and_code(angle_degrees * (100.0 / 360.0))
        munsell_spec = [hue, self.value / self.volume, self.chroma / self.volume, hue_code]

        return munsell_specification_to_munsell_colour(munsell_spec, 2, 2, 2)

    def __repr__(self):
        return f'CentroidColorAccumulator({self.value}, {self.chroma}, {self.hue_x}, {self.hue_y}, {self.volume})'


class ColorDatabase(object):
    def __init__(self, filename):
        self.load(filename)

    def load(self, filename):
        tree = ET.parse(filename)
        root = tree.getroot()

        # load colors
        self._level3_colors = {}
        centroid_color = {}
        for name_node in root.findall('./names/name/name/name'):
            color = NamedColor(int(name_node.get('color')), name_node.get('name'), name_node.get('abbr'))
            self._level3_colors[color.id] = color
            centroid_color[color.id] = CentroidColorAccumulator()

        self._hue_points = []

        self._color_ranges = []
        for hue_node in root.findall('./ranges/hue-range'):
            hue_begin = _get_hue_point(hue_node.get('begin'))
            self._hue_points.append(hue_begin)
            hue_end = _get_hue_point(hue_node.get('end'))
            for range_node in hue_node.findall('./range'):
                color_id = int(range_node.get('color'))
                value_begin = float(range_node.get('value-begin'))
                value_end = float(range_node.get('value-end'))
                chroma_begin = float(range_node.get('chroma-begin'))
                chroma_end = float(range_node.get('chroma-end'))
                new_block = ColorMatch(color_id, hue_begin, hue_end, value_begin, value_end, chroma_begin, chroma_end)
                self._color_ranges.append(new_block)
                centroid_color[color_id].append_block(new_block)

        for c in self._level3_colors.keys():
            self._level3_colors[c].centroid_color = centroid_color[c].get_centroid()


    def get_level3_colors(self):
        return self._level3_colors

    def get_color_ranges(self):
        return self._color_ranges

    def get_hue_points(self):
        return self._hue_points

    def get_descriptor_from_munsell(self, munsell_str):
        match_point, match_value, match_chroma = munsell_color_str_to_hvc(munsell_str)

        for m in self._color_ranges:
            matched = False
            if (m.chroma_begin <= match_chroma and match_chroma < m.chroma_end and
                m.value_begin <= match_value and match_value < m.value_end and
                _is_between_points(match_point, m.hue_begin, m.hue_end)):
                    matched = True

            if matched:
                return self._level3_colors[m.id]

        return None
