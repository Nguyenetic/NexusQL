"""Internal pure-Python geometry conversions: WKB <-> WKT <-> GeoJSON.

Supports the 2D (no Z/M) subset of simple-feature geometry types that
GeoPackage and GeoJSON actually use for facility-map data: POINT,
LINESTRING, POLYGON, MULTIPOINT, MULTILINESTRING, MULTIPOLYGON. No
external dependency (shapely, gdal, pyproj); this is deliberately small
and good enough for import/export round-tripping, not a general WKT/WKB
library.

Internal representation ("coords") per geometry type:
    POINT:           (x, y)
    LINESTRING:       [(x, y), ...]
    POLYGON:          [[(x, y), ...], ...]            # exterior, holes...
    MULTIPOINT:       [(x, y), ...]
    MULTILINESTRING:  [[(x, y), ...], ...]
    MULTIPOLYGON:     [[[(x, y), ...], ...], ...]      # list of POLYGON rings
"""

from __future__ import annotations

import struct
from typing import Any, List, Tuple

__all__ = [
    "read_wkb",
    "write_wkb",
    "wkt_from_geom",
    "geom_from_wkt",
    "geojson_from_geom",
    "geom_from_geojson",
]

_WKB_TYPE_TO_NAME = {
    1: "POINT",
    2: "LINESTRING",
    3: "POLYGON",
    4: "MULTIPOINT",
    5: "MULTILINESTRING",
    6: "MULTIPOLYGON",
}
_NAME_TO_WKB_TYPE = {v: k for k, v in _WKB_TYPE_TO_NAME.items()}


# --------------------------------------------------------------------------
# WKB
# --------------------------------------------------------------------------


class _Reader:
    __slots__ = ("buf", "pos", "endian")

    def __init__(self, buf: bytes) -> None:
        self.buf = buf
        self.pos = 0
        self.endian = "<"

    def u8(self) -> int:
        v = self.buf[self.pos]
        self.pos += 1
        return v

    def u32(self) -> int:
        v = struct.unpack_from(self.endian + "I", self.buf, self.pos)[0]
        self.pos += 4
        return v

    def f64(self) -> float:
        v = struct.unpack_from(self.endian + "d", self.buf, self.pos)[0]
        self.pos += 8
        return v

    def point(self) -> Tuple[float, float]:
        return (self.f64(), self.f64())

    def ring(self) -> List[Tuple[float, float]]:
        n = self.u32()
        return [self.point() for _ in range(n)]


def read_wkb(buf: bytes) -> Tuple[str, Any]:
    """Parse a standard (ISO-simple, 2D, no SRID flag) WKB blob.

    Returns (geometry_type_name, coords) using the module's internal
    coords representation.
    """
    r = _Reader(buf)
    byte_order = r.u8()
    r.endian = "<" if byte_order == 1 else ">"
    geom_type = r.u32() & 0xFF  # strip Z/M/SRID flag bits if present
    name = _WKB_TYPE_TO_NAME.get(geom_type)
    if name is None:
        raise ValueError(f"unsupported WKB geometry type code {geom_type}")

    if name == "POINT":
        return name, r.point()
    if name == "LINESTRING":
        return name, r.ring()
    if name == "POLYGON":
        n = r.u32()
        return name, [r.ring() for _ in range(n)]
    if name == "MULTIPOINT":
        n = r.u32()
        pts = []
        for _ in range(n):
            r.u8()
            r.u32()
            pts.append(r.point())
        return name, pts
    if name == "MULTILINESTRING":
        n = r.u32()
        lines = []
        for _ in range(n):
            r.u8()
            r.u32()
            lines.append(r.ring())
        return name, lines
    if name == "MULTIPOLYGON":
        n = r.u32()
        polys = []
        for _ in range(n):
            r.u8()
            r.u32()
            nr = r.u32()
            polys.append([r.ring() for _ in range(nr)])
        return name, polys
    raise ValueError(f"unsupported WKB geometry type {name}")  # pragma: no cover


def _pack_point(p: Tuple[float, float]) -> bytes:
    return struct.pack("<dd", p[0], p[1])


def _pack_ring(ring) -> bytes:
    out = struct.pack("<I", len(ring))
    for p in ring:
        out += _pack_point(p)
    return out


def write_wkb(geom_type: str, coords: Any) -> bytes:
    """Build a standard little-endian WKB blob (no SRID flag, 2D)."""
    name = geom_type.upper()
    code = _NAME_TO_WKB_TYPE.get(name)
    if code is None:
        raise ValueError(f"unsupported geometry type {geom_type!r}")
    header = struct.pack("<BI", 1, code)

    if name == "POINT":
        return header + _pack_point(coords)
    if name == "LINESTRING":
        return header + _pack_ring(coords)
    if name == "POLYGON":
        out = header + struct.pack("<I", len(coords))
        for ring in coords:
            out += _pack_ring(ring)
        return out
    if name == "MULTIPOINT":
        out = header + struct.pack("<I", len(coords))
        for p in coords:
            out += struct.pack("<BI", 1, 1) + _pack_point(p)
        return out
    if name == "MULTILINESTRING":
        out = header + struct.pack("<I", len(coords))
        for line in coords:
            out += struct.pack("<BI", 1, 2) + _pack_ring(line)
        return out
    if name == "MULTIPOLYGON":
        out = header + struct.pack("<I", len(coords))
        for poly in coords:
            inner = struct.pack("<I", len(poly))
            for ring in poly:
                inner += _pack_ring(ring)
            out += struct.pack("<BI", 1, 3) + inner
        return out
    raise ValueError(f"unsupported geometry type {geom_type!r}")  # pragma: no cover


# --------------------------------------------------------------------------
# WKT
# --------------------------------------------------------------------------


def _fmt_num(v: float) -> str:
    if v == int(v):
        return str(int(v))
    return repr(float(v))


def _fmt_point(p) -> str:
    return f"{_fmt_num(p[0])} {_fmt_num(p[1])}"


def _fmt_ring(ring) -> str:
    return "(" + ", ".join(_fmt_point(p) for p in ring) + ")"


def wkt_from_geom(geom_type: str, coords: Any) -> str:
    name = geom_type.upper()
    if name == "POINT":
        return f"POINT ({_fmt_point(coords)})"
    if name == "LINESTRING":
        return f"LINESTRING ({', '.join(_fmt_point(p) for p in coords)})"
    if name == "POLYGON":
        return "POLYGON (" + ", ".join(_fmt_ring(r) for r in coords) + ")"
    if name == "MULTIPOINT":
        return "MULTIPOINT (" + ", ".join(_fmt_point(p) for p in coords) + ")"
    if name == "MULTILINESTRING":
        return "MULTILINESTRING (" + ", ".join(
            "(" + ", ".join(_fmt_point(p) for p in line) + ")" for line in coords
        ) + ")"
    if name == "MULTIPOLYGON":
        polys = []
        for poly in coords:
            polys.append("(" + ", ".join(_fmt_ring(r) for r in poly) + ")")
        return "MULTIPOLYGON (" + ", ".join(polys) + ")"
    raise ValueError(f"unsupported geometry type {geom_type!r}")


class _WktParser:
    """A tiny hand-rolled WKT tokenizer/parser for the six supported types."""

    def __init__(self, text: str) -> None:
        self.text = text
        self.pos = 0
        self.n = len(text)

    def _skip_ws(self) -> None:
        while self.pos < self.n and self.text[self.pos] in " \t\r\n":
            self.pos += 1

    def _expect(self, ch: str) -> None:
        self._skip_ws()
        if self.pos >= self.n or self.text[self.pos] != ch:
            raise ValueError(f"expected {ch!r} at position {self.pos} in {self.text!r}")
        self.pos += 1

    def _peek(self) -> str:
        self._skip_ws()
        return self.text[self.pos] if self.pos < self.n else ""

    def _read_word(self) -> str:
        self._skip_ws()
        start = self.pos
        while self.pos < self.n and (self.text[self.pos].isalpha()):
            self.pos += 1
        return self.text[start:self.pos].upper()

    def _read_number(self) -> float:
        self._skip_ws()
        start = self.pos
        while self.pos < self.n and self.text[self.pos] in "+-0123456789.eE":
            self.pos += 1
        if start == self.pos:
            raise ValueError(f"expected number at position {self.pos} in {self.text!r}")
        return float(self.text[start:self.pos])

    def _read_point(self) -> Tuple[float, float]:
        x = self._read_number()
        y = self._read_number()
        return (x, y)

    def _read_ring(self) -> List[Tuple[float, float]]:
        self._expect("(")
        pts = [self._read_point()]
        while self._peek() == ",":
            self.pos += 1
            pts.append(self._read_point())
        self._expect(")")
        return pts

    def parse(self) -> Tuple[str, Any]:
        name = self._read_word()
        if name not in _NAME_TO_WKB_TYPE:
            raise ValueError(f"unsupported WKT geometry type {name!r}")
        self._skip_ws()
        if self.pos < self.n and self.text[self.pos : self.pos + 3].upper() == "EMPTY":
            return name, ([] if name != "POINT" else (float("nan"), float("nan")))

        if name == "POINT":
            self._expect("(")
            p = self._read_point()
            self._expect(")")
            return name, p
        if name == "LINESTRING":
            return name, self._read_ring()
        if name == "POLYGON":
            self._expect("(")
            rings = [self._read_ring()]
            while self._peek() == ",":
                self.pos += 1
                rings.append(self._read_ring())
            self._expect(")")
            return name, rings
        if name == "MULTIPOINT":
            self._expect("(")
            pts = []
            self._skip_ws()
            # MULTIPOINT accepts both "(1 2, 3 4)" and "((1 2), (3 4))"
            if self.pos < self.n and self.text[self.pos] == "(":
                pts.append(self._read_point_in_parens())
                while self._peek() == ",":
                    self.pos += 1
                    pts.append(self._read_point_in_parens())
            else:
                pts.append(self._read_point())
                while self._peek() == ",":
                    self.pos += 1
                    pts.append(self._read_point())
            self._expect(")")
            return name, pts
        if name == "MULTILINESTRING":
            self._expect("(")
            lines = [self._read_ring()]
            while self._peek() == ",":
                self.pos += 1
                lines.append(self._read_ring())
            self._expect(")")
            return name, lines
        if name == "MULTIPOLYGON":
            self._expect("(")
            polys = [self._read_polygon_body()]
            while self._peek() == ",":
                self.pos += 1
                polys.append(self._read_polygon_body())
            self._expect(")")
            return name, polys
        raise ValueError(f"unsupported WKT geometry type {name!r}")  # pragma: no cover

    def _read_point_in_parens(self) -> Tuple[float, float]:
        self._expect("(")
        p = self._read_point()
        self._expect(")")
        return p

    def _read_polygon_body(self) -> List[List[Tuple[float, float]]]:
        self._expect("(")
        rings = [self._read_ring()]
        while self._peek() == ",":
            self.pos += 1
            rings.append(self._read_ring())
        self._expect(")")
        return rings


def geom_from_wkt(text: str) -> Tuple[str, Any]:
    return _WktParser(text.strip()).parse()


# --------------------------------------------------------------------------
# GeoJSON
# --------------------------------------------------------------------------

_WKT_TO_GEOJSON_TYPE = {
    "POINT": "Point",
    "LINESTRING": "LineString",
    "POLYGON": "Polygon",
    "MULTIPOINT": "MultiPoint",
    "MULTILINESTRING": "MultiLineString",
    "MULTIPOLYGON": "MultiPolygon",
}
_GEOJSON_TO_WKT_TYPE = {v: k for k, v in _WKT_TO_GEOJSON_TYPE.items()}


def _coords_to_json(geom_type: str, coords: Any) -> Any:
    name = geom_type.upper()
    if name == "POINT":
        return [coords[0], coords[1]]
    if name in ("LINESTRING", "MULTIPOINT"):
        return [[p[0], p[1]] for p in coords]
    if name in ("POLYGON", "MULTILINESTRING"):
        return [[[p[0], p[1]] for p in ring] for ring in coords]
    if name == "MULTIPOLYGON":
        return [[[[p[0], p[1]] for p in ring] for ring in poly] for poly in coords]
    raise ValueError(f"unsupported geometry type {geom_type!r}")


def geojson_from_geom(geom_type: str, coords: Any) -> dict:
    name = geom_type.upper()
    gj_type = _WKT_TO_GEOJSON_TYPE.get(name)
    if gj_type is None:
        raise ValueError(f"unsupported geometry type {geom_type!r}")
    return {"type": gj_type, "coordinates": _coords_to_json(name, coords)}


def _json_to_coords(geom_type: str, coordinates: Any) -> Any:
    name = geom_type.upper()
    if name == "POINT":
        return (float(coordinates[0]), float(coordinates[1]))
    if name in ("LINESTRING", "MULTIPOINT"):
        return [(float(p[0]), float(p[1])) for p in coordinates]
    if name in ("POLYGON", "MULTILINESTRING"):
        return [[(float(p[0]), float(p[1])) for p in ring] for ring in coordinates]
    if name == "MULTIPOLYGON":
        return [
            [[(float(p[0]), float(p[1])) for p in ring] for ring in poly]
            for poly in coordinates
        ]
    raise ValueError(f"unsupported geometry type {geom_type!r}")


def geom_from_geojson(geojson_geom: dict) -> Tuple[str, Any]:
    gj_type = geojson_geom["type"]
    name = _GEOJSON_TO_WKT_TYPE.get(gj_type)
    if name is None:
        raise ValueError(f"unsupported GeoJSON geometry type {gj_type!r}")
    return name, _json_to_coords(name, geojson_geom["coordinates"])
