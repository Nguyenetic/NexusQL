# Geometry and metric spatial functions

`GEOMETRY` stores two-dimensional coordinates and optional SRID metadata. Input
may be WKT text, `ST_GeomFromText(wkt [, srid])`, `ST_Point(x, y)`, or
`ST_MakeEnvelope(xmin, ymin, xmax, ymax [, srid])`. Parameters use `$1`, `$2`, etc.
Coordinates use the map's units: a distance of 10 means 10 metres only when the
map coordinates are in metres. Use `ST_Transform` to convert longitude/latitude
to a metric CRS before planar measurements or buffers. Geographic distances
are also available directly in metres.

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
| `ST_AsGeoJSON(g)` | GeoJSON geometry text |
| `ST_GeomFromGeoJSON(text)` | Geometry from a GeoJSON geometry fragment; defaults to SRID 4326 |
| `ST_SRID(g)` | Stored SRID, or 0 when unspecified |
| `ST_SetSRID(g, srid)` | Copy with new SRID metadata; coordinates stay unchanged |
| `ST_Transform(g, to_srid)` | Reproject every coordinate and set the destination SRID |
| `ST_X(point)`, `ST_Y(point)` | Coordinate |
| `ST_Contains(a, b)` | b lies in a, with an interior intersection |
| `ST_Within(a, b)` | Same as `ST_Contains(b, a)` |
| `ST_Covers(a, b)` | Every point of b belongs to a, including its boundary |
| `ST_CoveredBy(a, b)` | Same as `ST_Covers(b, a)` |
| `ST_Intersects(a, b)` | Any shared point, including boundaries |
| `ST_Distance(a, b)` | Minimum planar distance |
| `ST_DistanceSphere(a, b [, radius])` | Minimum spherical distance between lon/lat geometries, in metres |
| `ST_DistanceSpheroid(a, b [, spheroid])` | Ellipsoidal distance between lon/lat geometries, in metres; WGS84 by default |
| `ST_DWithin(a, b, radius)` | Distance is at most radius (inclusive) |
| `ST_Area(g)` | Unsigned area, subtracting polygon holes |
| `ST_Length(g)` | Planar length of linear components; polygons and points contribute zero |
| `ST_Perimeter(g)` | Planar polygon boundary length, including holes; lines and points contribute zero |
| `ST_Centroid(g)` | Geometry centroid |
| `ST_Envelope(g)` | Bounding geometry: polygon, line, or point |
| `ST_Buffer(g, distance)` | Polygon/MultiPolygon at the given planar distance, with round joins and caps |
| `ST_Union(a, b)` | Geometric set union, dissolving overlaps |
| `ST_Union(g)` | Aggregate union across rows, including `GROUP BY` |
| `ST_IsValid(g)` | Whether the geometry satisfies 2D simple-feature validity |
| `ST_MakeValid(g)` | Repair invalid geometry, retaining collapsed line/point components |
| `ST_ClosestPoint(a, b)` | Point on a at the minimum planar distance to b |

A point on a polygon boundary is **not contained**, but it intersects the polygon.
It is also covered by the polygon. A hole's edge is covered; its interior is not.
Line endpoints are covered. Covers/CoveredBy return false if either geometry is
empty, and support multi-geometries and collections. Polygon holes are respected.
`ST_Contains(g, g)` can be true: this is containment,
not “contains properly.” Algorithms come from [geo](https://docs.rs/geo/0.33.1/geo/),
types from geo-types, and parsing/formatting from [wkt](https://docs.rs/wkt/0.14.0/wkt/).

WKT formatting is canonicalized; whitespace is not preserved. `SRID=n;WKT` input
also preserves the SRID. Binary geometry operations require matching SRIDs (0 and
unspecified are equivalent). Z/M coordinates and non-finite coordinates are not
supported. Malformed direct column input raises a query error. As with other
NexusQL scalar functions, invalid function arguments, unsupported geometry
conversions, and SRID mismatches return NULL. NULL inputs propagate. A negative
`ST_DWithin` radius returns false. Distance to empty geometry returns NULL.

## Coordinate systems and KFM measurements

`ST_Transform` supports EPSG:4326 (WGS84 longitude/latitude in **x/y degree order**),
EPSG:3857 (Web Mercator metres), EPSG:32601 through 32660 (WGS84 UTM north), and
EPSG:32701 through 32760 (WGS84 UTM south). Every pair of these CRSs is supported.
The pure Rust [proj4rs](https://docs.rs/proj4rs/0.1.10/proj4rs/) implementation uses
the sixth-order Poder/Engsager series for UTM. It adds no system PROJ or C build
dependency.

KFM near longitude -86.507, latitude 36.940 uses **EPSG:32616 (UTM 16N)**:

```sql
-- Approximately POINT(543899.795567 4088329.915872), in metres.
SELECT ST_AsText(ST_Transform(ST_SetSRID(ST_Point(-86.507,36.940),4326),32616));

-- A 10-metre buffer suitable for a longitude/latitude map client.
SELECT ST_AsGeoJSON(ST_Transform(
  ST_Buffer(ST_Transform(ST_SetSRID(ST_Point(-86.507,36.940),4326),32616),10),
  4326));
```

SetSRID only labels coordinates, so label their actual source CRS before
transforming. It accepts a signed 32-bit SRID, including 0 for unknown; Transform
requires a supported, nonzero source and destination. Unknown SRIDs, invalid
longitude/latitude ranges, non-finite results, and projection singularities
(such as Web Mercator at a pole) return NULL. A failed vertex fails the whole
geometry. Empty geometries retain their empty type and receive the destination
SRID. Collections, polygon holes, and all multi-geometries transform recursively;
edges are not densified. UTM accuracy applies in the zone's operating area.
Projected lengths are grid measurements; ellipsoidal point distances measure the
Earth's surface.

`ST_DistanceSphere` and `ST_DistanceSpheroid` accept points, lines, polygons,
multi-geometries, and collections. Both inputs must have matching SRIDs, either
4326 or unspecified/0, and coordinates within
longitude [-180,180] and latitude [-90,90]. Empty inputs return NULL; empty
members inside a nonempty collection are ignored. Intersections and containment
give zero distance. Polygon holes, antimeridian crossings, and poles are handled
on the sphere. Edges follow the shorter great-circle arc; a ring encloses the
smaller spherical region independently of winding. An edge with antipodal
endpoints has no unique arc and returns NULL.

Sphere uses the WGS84 mean radius, 6371008.771415059 metres. Its optional radius
must be positive and finite. Spheroid uses geo's pure Rust GeographicLib/Karney
inverse solution, which handles coincident points, the antimeridian, and
antipodal points. For lines and polygons it measures spherical closest-approach
candidates on the ellipsoid, following the approach in
[PostGIS's geographic distance implementation](https://github.com/postgis/postgis/blob/master/liblwgeom/lwgeodetic.c).
It does not reinterpret each edge as an ellipsoidal geodesic. Its optional text
argument has the form `'SPHEROID["WGS 84",6378137,298.257223563]'`: name,
semi-major radius in metres,
and inverse flattening. Radius must be positive and finite; inverse flattening
must be finite and greater than 1, or 0 for a sphere. These are scalar geometry
functions; a `geography` SQL type/cast is not added.

## Editing, validity, and measurements

Buffer, Union, MakeValid, and ClosestPoint preserve SRID metadata. Binary editing
requires matching SRIDs. Operations are 2D and use the input CRS's coordinate
units; buffering an EPSG:4326 geometry therefore uses degrees. Polygon overlays
use geo/i_overlay floating-coordinate topology. Output vertex order and component
order are not contractual; compare topology and measurements rather than WKT
ordering.

- **Buffer:** circular arcs use 8 segments per quadrant. Positive distances expand
  shapes; negative distances shrink polygons and return an empty polygon for
  points/lines. A polygon that disappears, and any empty input, produce
  `POLYGON EMPTY`. Zero-distance buffers return polygonal components only. Invalid
  inputs are repaired first. Only the two-argument form is supported; style,
  quadrant-count, and geography overloads are not implemented.
- **Union:** overlapping polygon areas dissolve, duplicate points disappear,
  lines split at crossings/overlap endpoints, and components covered by a higher
  dimensional component are removed. Mixed inputs can produce a
  GeometryCollection. Line segments are not sewn into a single LineString.
  Empty inputs contribute no points. Invalid inputs return NULL; repair them
  first. Two-argument union propagates NULL. Aggregate union skips NULL rows,
  returns NULL for empty/all-NULL input, and returns NULL for an invalid non-NULL
  input or mixed SRIDs. The existing executor supports a top-level aggregate
  (`SELECT ST_Union(geom) FROM rooms`) and grouped aggregates. Scalar wrappers
  around aggregates in an ungrouped SELECT, array inputs, and grid-size overloads
  are not implemented.
- **Validity:** checks include degenerate rings, self-intersections, outside or
  overlapping holes, overlapping MultiPolygon members, and disconnected polygon
  interiors. Self-crossing LineStrings can be valid; validity is not simplicity.
  Empty geometries are valid. MakeValid leaves valid inputs unchanged, deduplicates
  and nodes invalid ring linework, polygonizes it, and preserves leftover cut edges
  and collapsed points (including cut edges inside repaired areas).
  A bowtie can become a MultiPolygon; a collapsed ring can become linework or a
  point; mixed results can become a collection. Outside holes become separate
  polygon components. Repair returns NULL if a valid result cannot be constructed.
  WKT parsing already closes polygon rings, so IsValid examines that parsed shape.
- **Length and perimeter:** recursively sum the relevant dimensions of a
  collection. Empty geometries return zero. Closed LineStrings have length, but
  their perimeter is zero; polygons have perimeter, but their length is zero.
- **ClosestPoint:** returns a point on the **first** input, including a shared
  point when geometries intersect or one contains the other. Polygon holes and
  segment crossings are respected. Either empty input returns NULL. When several
  points tie for minimum distance, any one can be returned.

These choices follow the two-dimensional behavior of the PostGIS
[buffer](https://postgis.net/docs/ST_Buffer.html),
[union](https://postgis.net/docs/ST_Union.html), and
[validity repair](https://postgis.net/docs/ST_MakeValid.html) functions within the
overloads described here.

## GeoJSON interchange

`ST_GeomFromGeoJSON` accepts a geometry fragment: Point, LineString, Polygon,
MultiPoint, MultiLineString, MultiPolygon, or a nested GeometryCollection.
Feature and FeatureCollection documents return NULL. Coordinates must be finite
numeric pairs; Z/M coordinates, malformed JSON, one-coordinate LineStrings, and
unclosed/undersized polygon rings return NULL. Empty arrays represent empty
geometries except for Point, whose empty representation is not supported by the
engine. Parsing does not automatically repair self-intersections.

Without a CRS, input defaults to SRID 4326. Named `EPSG:n` and
`urn:ogc:def:crs:EPSG::n` CRS objects are accepted. AsGeoJSON produces compact JSON
with full floating-point precision and no implicit reprojection. Like the
[PostGIS default](https://postgis.net/docs/ST_AsGeoJSON.html), a known non-4326
SRID adds a short named CRS object. SRID 0/unspecified and 4326 omit that object;
thus exporting unlabelled data and importing it again assigns 4326. Transform to
4326 before exporting for RFC 7946 clients. Feature generation, decimal-place
options, and bounding-box options are not implemented.

## Index execution and maintenance

GiST on a declared geometry column builds an [rstar R-tree](https://docs.rs/rstar/0.13.0/rstar/struct.RTree.html).
GiST on other columns retains its range-tree behavior. Existing rows are indexed
when the index is created, including when it is created on an empty table.

`ST_Contains`, `ST_Within`, `ST_Covers`, `ST_CoveredBy`, `ST_Intersects`, and
`ST_DWithin` in WHERE can probe either geometry argument when it is an indexed
column and the other argument is
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

`tests/geometry_functions_test.rs` adds independent PROJ 9.8.1 reference
coordinates (including KFM, zone edges, both hemispheres and high latitudes),
submillimetre UTM error checks, all 120 UTM zone codes, and geodesic references
with tighter than 0.1% tolerance. It also checks coverage predicates against
scan results and real R-tree search counters, shared-edge INNER/LEFT joins,
editing/validity edge cases against GEOS 3.14.1 references, aggregate union, and
GeoJSON round trips. The reference coordinates are embedded constants; running
the tests requires no
Python, PROJ installation, or external service.
