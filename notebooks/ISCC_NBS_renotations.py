import colour
import munsell
import numpy as np
import math

from munsell import parse_munsell_colour, MUNSELL_COLOUR_FORMAT
from munsell import munsell_specification_to_munsell_colour
from munsell import munsell_colour_to_munsell_specification
from munsell import munsell_colour_to_xyY
from munsell import xyY_to_munsell_colour


def lch_to_rgb(lch):
    Lab = colour.LCHab_to_Lab(lch)
    XYZ = colour.Lab_to_XYZ(Lab)
    RGB = colour.XYZ_to_sRGB(XYZ)
    return colour.notation.RGB_to_HEX(RGB)


def lch_to_polar(lch):
    return (np.deg2rad(lch[2]), lch[1])


def lch_to_munsell(lch):
    orig_lch = lch
    for x in range(0, 20):
        try:
            Lab = colour.LCHab_to_Lab(lch)
            XYZ = colour.Lab_to_XYZ(Lab)
            xyY = colour.XYZ_to_xyY(XYZ, colour.CCS_ILLUMINANTS['CIE 1931 2 Degree Standard Observer']['C'])
            return xyY_to_munsell_colour(xyY)

        except Exception as e:
            exp = e
            c[1] = c[1] * 0.95
            continue

    raise exp
    return 'N0.0'

    
def munsell_to_lch(munsell_string):
    orig_munsell_string = munsell_string
    chroma_scale = 1.0
    for x in range(0, 2):
        try:
            xyY = munsell_colour_to_xyY(munsell_string)
            XYZ = colour.xyY_to_XYZ(xyY)
            C = colour.CCS_ILLUMINANTS['CIE 1931 2 Degree Standard Observer']['C']
            Lab = colour.XYZ_to_Lab(XYZ, C)
            lch = colour.Lab_to_LCHab(Lab)
            # for chromas that are past the limits of the munsell renotation, we
            # drop original chroma down to the maximum, but then scale the output lch
            # chroma linearly.
            lch[1] = lch[1] * chroma_scale
            return lch
        except Exception as e:
            exp = e
            hue, value, chroma, hue_code = munsell_colour_to_munsell_specification(munsell_string)
            chroma_maximum = munsell.maximum_chroma_from_renotation([hue, value, hue_code])
            chroma_scale = chroma / chroma_maximum
            munsell_string = munsell_specification_to_munsell_colour([hue, value, chroma_maximum, hue_code])
            continue
    raise exp


def hue_and_code_to_point(hue, hue_code):
    # put this hue and hue code on a 100-point scale, with 5Y at 90 degrees
    return (170.0 - ((hue_code)*10.0 - hue)) % 100.0


def point_to_hue_and_code(point):
    point = -point + 170.0
    hue = 10.0 - (point % 10.0)
    hue_code = ((point - (point % 10.0)) / 10.0) % 10.0 + 1.0
    return hue, hue_code


def munsell_to_polar(munsell_string):
    hue, _, chroma, hue_code = parse_munsell_colour(munsell_string)
    hue_point = hue_and_code_to_point(hue, hue_code)
    hue_angle = hue_point * (math.pi / 50.0)
    return hue_angle, chroma


def munsell_to_rgbstr(munsell_string):
    lch = munsell_to_lch(munsell_string)
    Lab = colour.LCHab_to_Lab(lch)
    XYZ = colour.Lab_to_XYZ(Lab)
    sRGB = colour.XYZ_to_sRGB(XYZ)
    return colour.notation.RGB_to_HEX(sRGB)


def set_xticks_munsell_hues(ax):
    xtick_major_labels = ['5{}'.format(x) for x in munsell.MUNSELL_HUE_LETTER_CODES.keys()]
    xtick_major_ticks = [munsell_to_polar('{} 0.0/0.0'.format(x))[0] for x in xtick_major_labels]
    xtick_minor_labels = (['10{}'.format(x) for x in munsell.MUNSELL_HUE_LETTER_CODES.keys()])
    xtick_minor_ticks = [munsell_to_polar('{} 0.0/0.0'.format(x))[0] for x in xtick_minor_labels]

    ax.set_xticks(xtick_major_ticks, labels=xtick_major_labels, minor=False)
    ax.set_xticks(xtick_minor_ticks, labels=xtick_minor_labels, minor=True)
