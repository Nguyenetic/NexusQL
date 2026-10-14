# Planar geometry

`GEOMETRY` stores two-dimensional coordinates and optional SRID metadata. Input
may be WKT text, `ST_GeomFromText(wkt [, srid])`, `ST_Point(x, y)`, or
`ST_MakeEnvelope(xmin, ymin, xmax, ymax [, srid])`. Parameters use `$1`, `$2`, etc.
Coordinates use the map's units: a distance of 10 means 10 metres only when the
map coordinates are in metres. There is no reprojection or geographic distance.

```sql
CREATE TABLE zones (code TEXT, geom GEOMETRY);
CREATE TABLE devices (name TEXT, geom GEOMETRY);
INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0, 0, 10, 10));
INSERT INTO devices VALUES ('CAM-01', ST_Point(2, 3));
CREATE INDEX zones_geom ON zones USING gist (geom);
CREATE INDEX devices_geom ON devices USING gist (geom);

SELECT d.name, z.code
FROM devices d JOIN zones z ON ST_Contains(z.geom, d.geom);

SELECT name FROM devices WHERE ST_DWithin(geom, ST_Point(0, 0), 10);
SELECT name, ST_Distance(geom, ST_Point(0, 0)) AS distance
FROM devices ORDER BY distance LIMIT 1;
```

## Functions and semantics

| Function | Result |
| --- | --- |
| `ST_AsText(g)` | WKT without SRID |
| `ST_SRID(g)` | Stored SRID, or 0 when unspecified |
| `ST_X(point)`, `ST_Y(point)` | Coordinate |
| `ST_Contains(a, b)` | b lies in a, with an interior intersection |
| `ST_Within(a, b)` | Same as `ST_Contains(b, a)` |
| `ST_Intersects(a, b)` | Any shared point, including boundaries |
| `ST_Distance(a, b)` | Minimum planar distance |
| `ST_DWithin(a, b, radius)` | Distance is at most radius (inclusive) |
| `ST_Area(g)` | Unsigned area, subtracting polygon holes |
| `ST_Centroid(g)` | Geometry centroid |
| `ST_Envelope(g)` | Bounding geometry: polygon, line, or point |

A point on a polygon boundary is **not contained**, but it intersects the polygon.
Polygon holes are respected. `ST_Contains(g, g)` can be true: this is containment,
not “contains properly.” Algorithms come from [geo](https://docs.rs/geo/0.33.1/geo/),
types from geo-types, and parsing/formatting from [wkt](https://docs.rs/wkt/0.14.0/wkt/).

WKT formatting is canonicalized; whitespace is not preserved. `SRID=n;WKT` input
also preserves the SRID. Binary geometry operations require matching SRIDs (0 and
unspecified are equivalent). Z/M coordinates and non-finite coordinates are not
supported. Malformed direct column input raises a query error. As with other
NexusQL scalar functions, invalid function arguments, unsupported geometry
conversions, and SRID mismatches return NULL. NULL inputs propagate. A negative
`ST_DWithin` radius returns false. Distance to empty geometry returns NULL.

## Index execution and maintenance

GiST on a declared geometry column builds an [rstar R-tree](https://docs.rs/rstar/0.13.0/rstar/struct.RTree.html).
GiST on other columns retains its range-tree behavior. Existing rows are indexed
when the index is created, including when it is created on an empty table.

`ST_Contains`, `ST_Within`, `ST_Intersects`, and `ST_DWithin` in WHERE can probe
either geometry argument when it is an indexed column and the other argument is
constant or parameterized. DWithin expands the search box by the radius. AND
may use either indexed conjunct; OR is pruned only if both sides have an access
path. Row-dependent radii fall back to a scan. Candidate rows always pass through
the original exact SQL predicate.

INNER and LEFT joins probe indexes on the right table using geometry from each
outer row. Other joins fall back to ordinary evaluation. To accelerate a spatial
join, put the indexed table on the right, as in the example. Nearest-neighbor
`ORDER BY ST_Distance(...)` sorts exact distances; it does not use a KNN index path.

INSERT (including Python/direct bulk insert) adds entries incrementally. UPDATE,
DELETE, and replacement rebuild GiST entries because row IDs are vector offsets;
this favors correctness for small facility datasets. Geometry schema and GiST
definitions are persisted in a sidecar metadata key, so existing table metadata
remains readable. Indexes rebuild on reopen and are included in transaction
snapshots for rollback.

`GiSTIndex::stats()` exposes actual `search_count` and `candidates_returned`
counters. `tests/spatial_index_test.rs` checks them, verifies the three-zone/six-
device fixture, and compares a 10,000-point/500-polygon indexed join with a full
scan, including all shared-edge exclusions.

## Indexed nearest-neighbor queries

Use the PostGIS-style `<->` operator with an ascending `ORDER BY` and `LIMIT`
to find the closest indexed geometries:

```sql
CREATE TABLE switch_closets (name TEXT, in_service BOOLEAN, geom GEOMETRY);
CREATE INDEX closets_geom ON switch_closets USING gist (geom);

-- $1 and $2 are the camera's map coordinates.
SELECT name, geom <-> ST_Point($1, $2) AS distance
FROM switch_closets
WHERE in_service = true
ORDER BY geom <-> ST_Point($1, $2)
LIMIT 5;
```

For geometry operands, `<->` returns exactly the same planar distance as
`ST_Distance`, including its SRID checks and NULL behavior. It also works in
SELECT expressions and WHERE predicates. Its existing vector-distance meaning
is preserved for vector operands. `ORDER BY distance` using a SELECT alias,
qualified columns, and reversed operands can also use the geometry index.
The other operand must be independent of the scanned row; literals, parameters,
casts, and expressions such as `ST_Centroid(...)` are supported. It may be a
point, line, or polygon.

The scan pulls candidates lazily from rstar's nearest-neighbor iterator and
checks distances against the original geometries, including polygon holes.
For a query shape larger than a point, a circle enclosing its bounds supplies
a conservative stopping bound. Bounding-box or center distances never decide
the returned order. The scan keeps the best k exact matches and continues past
rows rejected by WHERE until enough matches are found or the index is exhausted.
Candidates that could tie the kth distance are checked too; equal distances
retain the same table-row order as the unindexed stable sort.

Without a geometry GiST index or a LIMIT, execution uses the full sort.
Descending order, row-dependent query geometries, joins, grouping, aggregate or
window queries, and WHERE subqueries also retain their existing execution path.
Tables containing NULL/empty geometries or mixed SRIDs, and targets with an
incompatible SRID or no geometry bounds, fall back to preserve existing NULL
sorting behavior. `ORDER BY ST_Distance(...)` continues to use the full sort.
Selective filters, many ties, or widely overlapping bounds can require visiting
most or all indexed entries.

`GiSTIndex::stats().search_count` includes KNN scans; `candidates_returned` counts
entries actually pulled, including a stopping lookahead. The KNN tests compare
indexed results with both the unindexed operator and `ST_Distance` over 10,000
seeded random points and polygons (including holes), for k = 1, 5, and 50, with
and without WHERE. Four query shapes produce 24 comparisons, each asserting one
index search and fewer than 2,500 candidate visits.
