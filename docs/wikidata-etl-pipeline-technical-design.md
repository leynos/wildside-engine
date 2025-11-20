# Wikidata ETL Pipeline Technical Design

## Input Data Sources

The pipeline will ingest data from two primary sources: **(1)** the full
**Wikidata JSON dump** (a highly compressed JSON file on the order of hundreds
of GB) and **(2)** a curated set of **Wikidata QIDs** corresponding to Points
of Interest already identified in OpenStreetMap (OSM). The OSM ingestion phase
produces a list of POIs (stored in an SQLite `pois.db` or similar) including
any `wikidata` tags. These tags provide the QIDs (entity IDs like Q12345) that
serve as a filter for relevant Wikidata
entries.[^design-285-293][^design-302-308] Before processing the dump, the
pipeline will load all such QIDs into an in-memory hash set for quick
membership checks. This avoids unnecessary work on unrelated entities by
allowing the parser to **skip the vast majority of Wikidata items** and focus
only on those linked via OSM. The Wikidata dump itself is a single giant JSON
array of entity objects (each entity on a separate line), typically distributed
as a compressed `.bz2` or `.gz`
file.[^qwikidata-dump-overview][^qwikidata-database-download] The ETL process
will stream through this file (decompressing on the fly) and parse it
line-by-line, rather than loading it entirely into memory, to handle the
massive size efficiently. Each JSON line (entity) begins with an `"id"` field;
the parser will extract this ID and quickly check if it is in the target QID
set. Non-matching entities are immediately discarded without full parsing,
significantly reducing CPU and memory load.

For each matching entity (i.e. a Wikidata item whose QID is in our OSM-derived
set), the pipeline will fully decode its JSON structure to access the data of
interest. The Rust **`wikidata` crate** (also referred to as **wikidata-rust**)
provides data structures for Wikidata entities and claims, and can be used to
deserialize JSON into Rust types representing entities, claims, and
values.[^design-316-320] This crate ensures the JSON fields (like labels,
descriptions, claims, etc.) are interpreted according to the Wikidata data
model. In summary, **the input stage filters and extracts only the relevant
Wikidata entries** by leveraging the existing OSM–Wikidata links, thereby
narrowing the processing from ~100 million entities to a much smaller subset
(the POIs of interest).

## Output Data Store (Intermediate)

The output of this ETL will be a **structured, queryable intermediate
database** that holds the enriched Wikidata information for each POI. For the
initial implementation, we will use a local **SQLite** database (file-based) to
store this data.[^design-294-301][^design-313-320] SQLite offers a
self-contained, zero-setup solution that integrates well with Rust (via the
`rusqlite` crate) and our existing `diesel` ORM setup. In fact, the Wildside
engine’s design already anticipates an offline artefact `pois.db` – an SQLite
file containing enriched POI data indexed for fast lookup.[^design-533-541] By
writing the ETL results to SQLite, we ensure the data can be accessed with
sub-millisecond query latency from local disk or memory-mapped pages, with
minimal overhead in production. The schema will be optimized for read-heavy
workloads, treating this store as essentially read-only once built (suitable
for the engine’s offline/online separation[^design-529-537]).

**Alternative formats** will be evaluated for future iterations. Two notable
options are **Apache Parquet** (a columnar on-disk format) and **Apache Arrow**
(an in-memory columnar format). Parquet could reduce disk footprint and improve
analytical query performance by storing data in a compressed columnar layout
(useful if performing large scans or aggregations on the data). Arrow could
facilitate zero-copy data sharing between processes or enable in-memory
analytics with libraries like DataFusion or Polars. However, these formats come
with added complexity – Parquet requires a writer and adds a conversion step
when loading into Postgres, and Arrow is more suitable for in-memory processing
than long-term storage. For the MVP, SQLite is preferred due to its simplicity
and the ability to use standard SQL queries. It also aligns with the
`wildside-data` approach of having a lightweight embedded store. We will keep
the design flexible so that swapping out SQLite for **RocksDB** (a
high-performance key-value store) or outputting additional formats is possible
behind a feature flag[^design-294-302]. (RocksDB could be considered if write
throughput or concurrent reads become bottlenecks, but SQLite with proper
indexing is expected to suffice initially.)

Regardless of format, the **content** of the intermediate store remains the
same: each relevant POI’s Wikidata claims are extracted and stored in a
structured manner. This intermediate database will later be used to enrich the
main Postgres `pois` table. It is essentially a staging area that allows
verification, fast local queries, and possible reuse for other computations
(e.g. computing popularity metrics) before the data is loaded into the
production database.

## Extracting and Transforming Key Wikidata Claims

The core transformation step of the ETL is to **extract specific claims** from
each matched Wikidata entity and normalize their values into typed fields. We
focus on a set of high-value properties (claims) for the MVP, chosen for their
relevance to Points of Interest:

- **P1435 (heritage status)** – e.g. national heritage designation or UNESCO
  World Heritage status. These values reference items (QIDs) indicating the
  type of heritage designation. The parser will collect all heritage
  designations for the item. For example, a castle might have P1435 = Q916333
  (Scheduled Monument) or even multiple values. We will store the list of
  heritage QIDs for each POI, and we may flag if a top-tier designation like
  UNESCO (`Q9259`) is present.[^design-380-388] This could be stored as an
  array of QIDs or a boolean flag for UNESCO in addition to the list.

- **P31 (instance of)** – the general type of the entity (e.g. museum, church,
  monument). Often a single value, but items can have multiple P31 statements.
  These will be stored as references to other entities (QIDs). We will retain
  all instance-of QIDs to fully capture the types of the POI.

- **P279 (subclass of)** – hierarchical categories or classes the entity falls
  under. This typically applies if the item itself is a class; however, some
  POIs might include subclass relationships. If present, we will capture these
  QID references as well. Together, P31 and P279 give a taxonomy of the POI
  (e.g. a POI might be an instance of “art museum” which is a subclass of
  “museum”).

- **P18 (image)** – a representative image filename from Wikimedia Commons.
  This is a string (the name of the image file, e.g. `"Eiffel_Tower.jpg"`). The
  pipeline will extract the filename (or potentially form a full URL to the raw
  image via Wikimedia’s image URL pattern), storing it as a text field. This
  can later be used to display images of the POI.

- **P1619 (date of official opening)** – the opening date of the POI (often for
  public venues, e.g. a museum’s opening date). Wikidata encodes dates with
  precision; we will parse this into a standard date or year. For simplicity,
  we might take the year as an integer (if full precision is not needed in
  queries), or store the full ISO date string if available. Normalising to an
  **integer year** or SQL date type makes it easier to filter or sort by age.

- **P571 (inception)** – the inception or founding date of the POI (often
  similar to opening date, but can also apply to when a building or institution
  was founded). We will treat this similarly to P1619: extract the value and
  store it as a year or date. These dates will be stored in a numeric or date
  column in the intermediate SQLite (e.g. an integer year field
  `inception_year`).

- **P856 (official website)** – the URL of the official website of the POI (if
  it exists). This is typically a URL string. We will store it as text (with a
  URL format). It’s an external link that can enrich the POI detail page or be
  used for reference.

- **P373 (Commons category)** – the name of a Wikimedia Commons category
  related to the POI. This is a string (often the category name). It can be
  used to fetch related media or further images of the POI. We will store this
  as text. (Note: A Commons category often groups images of the place; having
  this can allow fetching multiple images or media in the future.)

- **P2044 (elevation)** – the elevation above sea level, typically recorded as
  a quantity with a numerical value (and a unit, usually metres). The parser
  will extract the numeric value (converting units to a standard unit if
  necessary, likely metres). We will store elevation as an integer (e.g. 450
  metres) or a floating-point number if precision requires. This provides
  topographic context for the POI.

- **P625 (coordinate location)** – the geographic coordinates of the POI. Since
  our POIs already have coordinates from OSM, this might be redundant; however,
  it serves as a cross-check or a source of elevation (if altitude is included)
  or precision. Wikidata coordinates are given as latitude/longitude (and
  potentially altitude and reference globe). We will parse out latitude and
  longitude. If OSM provides coordinates, those are likely more up-to-date for
  location, but we can compare or use Wikidata coordinates if needed. These
  will be stored as numeric latitude and longitude fields. (If we choose, we
  might skip storing P625 since OSM covers coordinates, but for completeness
  and potential data validation, it can be included.)

**Data Normalisation:** Each extracted claim will be converted to a suitable
**type** before storage.[^design-290-298] For example, dates like P1619/P571
are stored in Wikidata as timestamp strings plus metadata; we will convert them
to a standard date or year number. URLs (P856) are taken as-is (string). QID
references (P1435, P31, P279, etc.) will be stored as the QID string or a
numeric ID representing that QID. We may use the Wikidata QID numeric encoding
(e.g. store `Q12345` as integer 12345 in a numeric field) for
compactness.[^wd2sql-id-structure][^wd2sql-see] The intermediate design favours
**explicit typing**: for instance, elevation will go into a numeric column
(allowing range queries), and images/websites remain text. Where a property can
have multiple values (e.g. multiple P31 or P1435 entries), the SQLite schema
might represent this as a JSON array or a separate linked table, to keep the
data normalized. A straightforward approach for MVP is to store multi-valued
properties as **JSON text in a column** (since SQLite’s JSON support allows
querying inside JSON if needed). For example, a column `instance_of` could
contain `["Q33506","Q12345"]` as a JSON array of QIDs. This retains structure
and is indexable via SQLite’s JSON functions if necessary. Alternatively, we
could use a separate table `poi_types` (poi_id, type_qid) with one row per P31
value, but given the limited scope of properties, embedding an array may
suffice for now.

**Linking to OSM POIs:** Each Wikidata entity in the filtered set corresponds
to one or more OSM POIs. In many cases, a unique OSM feature has the wikidata
tag linking to one QID. However, it is possible (though uncommon) that multiple
OSM objects reference the same Wikidata item (e.g. a building way and a node
both tagged with the same QID). To maintain this linkage, the ETL pipeline will
reference the OSM POI’s internal ID when storing the Wikidata data. The
simplest strategy is to include a **foreign key** or reference in the SQLite
schema: e.g. a column for `osm_poi_id` (matching the primary key of the POI in
the `pois` table). As the parser processes each entity, it can look up which
OSM POI(s) correspond to that QID (using a map from QID to OSM IDs prepared
from the OSM import). If multiple POIs share a QID, we may either duplicate the
Wikidata info for each POI record or have a join table. For the MVP, we can
treat the relationship as one-to-one (assuming each QID maps to a single POI)
and later accommodate one-to-many if needed. The **SQLite schema** might
include an `osm_id` on the Wikidata table, allowing efficient join queries
like: “find all heritage status values for POI with id = X”. Alternatively, we
could merge the data directly into the `pois` table (adding new columns for
each Wikidata property or an `enriched_data` JSON column per POI). The design
favours keeping the Wikidata-derived data in a dedicated structure initially,
to clearly delineate source and to avoid bloating the core POI record with
sparse columns. In any case, by linking back via IDs, we ensure the enriched
data can be **attached to the right POI** when loading into Postgres.

## Performance and Incremental Update Considerations

**Efficient Parsing:** Processing a multi-hundred-gigabyte JSON file is a
significant engineering challenge. The pipeline must be highly efficient in
parsing JSON and writing to the database. We will leverage Rust libraries for
performance: the **`simd-json`** crate provides a SIMD-accelerated JSON parser
that can dramatically speed up parsing of large text files.[^design-294-301]
Using `simd-json` (or the underlying simdjson algorithm) allows parsing to
approach I/O-bound speeds – in fact, tools like `wd2sql` have shown they can
parse Wikidata dumps nearly as fast as they can be decompressed from
bzip2.[^wd2sql-available][^wd2sql-ram] The ETL will likely use a streaming
pattern: for example, using a decompression library or command (`bzcat`) to
stream the dump into Rust, then using buffered reading to supply chunks of JSON
data to the parser. Each line (one entity) can be parsed either by `simd-json`
into a JSON value, or by using a custom streaming deserializer (perhaps using
`serde_json` in combination with simdjson’s parser). The **`wikidata` crate**
can assist by providing `serde::Deserialize` implementations for the entity and
claim structures.[^wikidata-serialization-note][^wikidata-claim-value] We will
evaluate if using serde with `simd-json` is feasible (there is a
`simd-json::to_typed` that can accelerate direct deserialization). The goal is
to minimize overhead per entity: skip irrelevant ones quickly, and for relevant
ones, parse only the needed substructures (claims of interest, and possibly the
sitelinks count or label if needed) rather than materializing the entire entity
with all languages and properties.

**Batch Inserts:** Writing to SQLite will be done in bulk transactions to
improve throughput. Instead of inserting one row per entity in autocommit mode
(which would be very slow), the pipeline will use a single transaction for a
large batch (or periodic batches, e.g. 1000 inserts per transaction) to
amortize commit cost. The `rusqlite` crate allows prepared statements for
insertion; we will prepare an insert statement for the Wikidata claims table
and reuse it for each entity, binding the extracted values. This approach is
inspired by `wd2sql`’s use of batched transactions and prepared statements to
achieve high insert rates.[^wd2sql-ram] Additionally, we can **disable SQLite’s
synchronous mode** or tune the journaling mode (e.g. use Write-Ahead Logging)
during the bulk load to further speed up inserts, since this is an offline
one-time load.

**Parallelism:** Although the JSON dump is essentially one large text file, we
can exploit parallelism in a couple of ways. One approach is to spawn multiple
threads that each parse a portion of the file – for example, splitting the file
by byte ranges or by lines. Because each entity JSON is self-contained (and
each line in the dump is an independent JSON object), we can assign segments of
the file to different threads to parse concurrently. Another approach is a
producer-consumer model: one thread (or an IO async task) reads and
decompresses lines, then a pool of worker threads takes lines from a queue and
parses/inserts them. Care must be taken to maintain ordering only insofar as
needed for determinism; otherwise, processing can be largely asynchronous.
Rust’s ownership and thread safety guarantees (and the Send/Sync on our data
structures) will allow safe parallel processing. The `wildside-engine` design
already highlights that a fast, parallel parser is essential for Wikidata
ingestion.[^design-290-298] By using all available CPU cores for JSON parsing
and processing, we can drastically cut down the wall-clock time to build the
database. For example, if one core can process ~30 MB/s of JSON, an 8-core
setup could in theory handle ~240 MB/s if I/O and decompression keep up, making
the difference between a multi-day run and a few hours.

**Memory Mapping:** As an alternative or complement to streaming,
memory-mapping the decompressed dump file can provide fast access to the bytes
and allow the simdjson algorithm to use pointer arithmetic on the raw memory.
If disk space permits, we might opt to decompress the entire dump to a binary
file and use `memmap2` to map it into memory for parsing. This avoids copying
data from kernel to user space repeatedly and can yield speed-ups, but requires
very large disk and virtual address space for the full dump (not always
feasible). Given the pipeline’s expected use (likely on a server or cloud
instance for data prep), we can consider this if it simplifies parsing logic.

**Incremental Updates:** Wikidata is a dynamic dataset with frequent changes.
The initial pipeline processes a full snapshot (e.g. the latest weekly dump),
which means the data can become stale until the next run. The design should
consider how to update the data incrementally. **In practice, incremental JSON
dumps are not provided by Wikidata** (there are no official JSON diff files),
making efficient sync challenging[^topicseed-hard]. One option is to
periodically re-run the full ETL on new dumps (e.g. monthly or weekly), which
is simpler but resource-intensive. Another is to consume Wikidata’s
**RecentChanges API** or incremental RDF dumps to catch updates between full
runs[^topicseed-hard][^topicseed-recent-changes]. For now, we will **evaluate
the feasibility of applying diffs**: Wikidata offers **daily or hourly
incremental dumps in RDF** and a RecentChanges feed, but integrating those
would add complexity (converting RDF changes to our SQLite format). A pragmatic
solution is to store the **dump version or timestamp** in our database (e.g. in
a metadata table with the date of the dump processed). This way, we at least
record the data currency. Future enhancements could involve a smaller job that
fetches changes since that timestamp and updates the SQLite accordingly. The
pipeline design will keep this in mind by modularizing the parsing logic (so it
can be reused for both full ingestions and smaller update sets). Initially,
however, the focus is on **idempotent full ingestions** – the offline run can
be scheduled during low-traffic periods, and since the data artefacts are
read-only at runtime, swapping in a newly generated `pois.db` is
straightforward.

Finally, we will implement **logging and progress tracking** given the long
processing time. The ETL should periodically log its progress (e.g. number of
entities processed) and perhaps write metrics (like how many entities matched
the filter, how many of each property were found, etc.). This ensures we can
monitor performance and detect any anomalies (for instance, if the QID filter
was too broad or if parsing slows down due to some pathological case). The
entire ETL is run via the `wildside-cli` offline path, making it easy to rerun
or automate.

## Integration with Postgres and Enrichment of the `pois` Table

Once the intermediate store (SQLite `pois.db`) is populated with the extracted
Wikidata data, the final step is to **load this enriched information into the
main Postgres database** that powers the Wildside application. The `pois` table
in Postgres currently holds the core POI data (id, coordinates, tags, etc.)
mostly derived from OSM. We need to attach the new semantic attributes to these
POIs. There are a couple of design approaches to this integration:

- **JSONB Enriched Column:** One straightforward method is to add a new column
  to the `pois` table, for example `enriched_data JSONB`, which will store a
  JSON object of all the Wikidata-derived fields for that POI. Each POI that
  has a Wikidata link would get a JSON object containing keys like
  `"heritage_status"`, `"instance_of"`, `"opening_date"`, etc., with values as
  extracted (scalar or arrays depending on the property). POIs without Wikidata
  data can have this column as NULL or an empty JSON. This approach has the
  advantage of flexibility – adding more properties in the future doesn’t
  require altering the table schema, and the JSON structure can mirror the data
  closely. PostgreSQL supports indexing JSONB data (e.g. GIN indexes for
  containment queries), so we could index certain keys if needed (like queries
  for all POIs with a certain heritage status). The Wildside engine might
  mostly fetch the entire JSON blob per POI and use it in application logic,
  which is efficient. This approach keeps the schema **backwards-compatible**
  and minimal, aligning with the design principle that `pois.db` (and by
  extension the Postgres schema) can evolve without breaking
  changes[^design-533-541].

- **Normalized Auxiliary Tables:** Another approach is to create additional
  tables in Postgres to hold the Wikidata attributes. For example, a table
  `poi_wikidata` with columns (`poi_id` (FK), `qid`, `heritage_statuses` (array
  of text QIDs), `instance_of` (text[]), `subclass_of` (text[]), `image_name`
  (text), `opening_date` (date), `inception_date` (date), `website` (text),
  `commons_category` (text), `elevation` (int), `coord` (geography) …). This
  makes each field directly queryable and type-safe. In cases where one POI has
  multiple QIDs (if that were possible, though typically not), the table could
  even allow multiple rows or have a one-to-one with `pois` and allow arrays in
  columns for multivalued fields. The benefit here is easier SQL querying on
  specific attributes (e.g. find all POIs opened before 1900, or all museums
  (`instance_of` contains Q33506)). It also avoids duplicating data for
  multi-valued fields by using proper relational design (e.g. a separate join
  table `poi_instance_of(poi_id, class_qid)` could list each instance-of
  relationship as a row). The downside is an increase in complexity – multiple
  joins would be needed to gather all info for one POI, and the schema becomes
  more complex to migrate. Given that our use-cases in the near term mostly
  involve reading all attributes for a POI (when presenting it or scoring it)
  and occasionally checking if a POI has a certain property, the JSONB column
  approach is appealing for its simplicity.

In either case, the **Diesel** ORM or raw SQL COPY can be used to load the
data. We will provide a Diesel **migration** that adds the necessary column(s)
or tables to the schema to accommodate the new data. For example, a migration
could add `ALTER TABLE pois ADD COLUMN enriched_data JSONB;` (if JSONB
approach) or create a new table. After migrating the schema, the ETL pipeline
will handle populating the data. One plan is to extend the
`wildside-cli ingest` command to, after building the SQLite, also perform a
load into Postgres. The CLI can open the SQLite `pois.db` (using `rusqlite`)
and query all enriched records, then use Diesel or bulk INSERTs to update the
Postgres DB. If using JSONB column, it might construct the JSON per POI and do
a single `UPDATE pois SET enriched_data = <json> WHERE id = ...`. If
performance is a concern (for tens of thousands of POIs, which is manageable),
we could generate a temporary CSV or SQL script from SQLite and use `psql COPY`
to load it efficiently. The **final output** is that the Postgres `pois` table
is enriched with semantic data from Wikidata, accessible to the application’s
query engine. This enriched data will enable new features – for instance,
filtering or boosting tour recommendations based on heritage status or opening
dates, showing images in the UI, etc., all powered by local data rather than
live Wikidata queries.

Throughout this integration, consistency with the rest of the architecture is
kept in mind. The offline pipeline ensures that `pois.db` and associated
artefacts are **authoritative** for all read queries at
runtime.[^design-412-416][^design-302-308] The engine’s scoring component, for
example, will consult the local SQLite/JSONB for properties like P1435 or P31
when computing user relevance scores on the fly.[^design-404-412] By having
this data readily available in a local store, the engine can perform
**thousands of property lookups per request in milliseconds** without any
external API calls.[^design-312-320][^design-413-416] This fulfills the
performance requirement that motivated the offline ETL in the first place: all
Wikidata enrichment data is preloaded and indexed, making query latency very
low (sub-millisecond per lookup) and throughput high.[^design-313-320] The
Postgres integration must therefore preserve that efficiency (likely by
ensuring that the web service can join or fetch the needed data with minimal
overhead, possibly caching some of it in memory as well). In summary, the
enriched metadata will be attached to POIs in Postgres in a form that is easy
to query and consistent with the existing data model, completing the pipeline
from raw dump to application-ready data.

## Roadmap: **Build Wikidata ETL Pipeline**

To implement this design, a series of development tasks will be executed
(tracked as the “Build Wikidata ETL Pipeline” phase in the project roadmap).
The work breakdown is as follows:

- **Add Wikidata Dependencies:** Update the `wildside-data` crate’s
  `Cargo.toml` to include new dependencies: the `wikidata` crate for Wikidata
  data structures, `simd-json` for high-performance JSON parsing, and
  `rusqlite` for SQLite database access[^design-313-320]. These libraries are
  chosen for their performance and functionality – for example, `simd-json` can
  significantly speed up parsing of the large dump, and `rusqlite` provides a
  safe wrapper around SQLite C APIs. Ensure that all added crates are
  permissively licensed (Apache/MIT) to satisfy project licensing requirements
  (the `wikidata` crate is Apache-2.0 licensed[^wikidata-license], and others
  are MIT/Apache dual-licensed), aligning with the codebase’s ISC license
  policy. This step also involves enabling any necessary feature flags (for
  instance, `rusqlite`’s `bundled` feature if we want to compile with an
  internal SQLite, or enabling JSON1 extension support if needed). With these
  dependencies in place, the project can compile support for the Wikidata ETL
  functionality.

- **Download Latest Wikidata Dump:** Write a utility (within `wildside-data` or
  as part of `wildside-cli`) to fetch the latest Wikidata JSON dump from the
  official Wikimedia dumps site. This could be a Rust function using `reqwest`
  to download the file, or simply a documented step to obtain the dump
  manually. For automation, the script will retrieve the dump URL (e.g.
  `https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.bz2`) and
  download it to a specified location. Given the size of the file, consider
  providing resume support or at least a progress indicator. If bandwidth or
  storage is a concern, an alternative is to stream the dump directly from the
  internet into the parser, but it’s safer to download fully so we can retry
  parsing without re-downloading. The script should also verify the integrity
  of the downloaded file (dumps often come with a SHA1/MD5 checksum). This task
  results in a local copy of the Wikidata dump file (compressed or
  decompressed). If working with the compressed `.bz2` directly, the subsequent
  parser can read from it using an appropriate decompression stream.

- **Implement Wikidata JSON Parser:** In the `wildside-data` module, develop a
  parsing pipeline that iterates through the dump and extracts the relevant
  data. This involves multiple sub-steps:

- **Stream Reading:** Open the dump file (through a bzip2 decoder if still
  compressed) and create an iterator over lines (each line representing one
  JSON entity)[^qwikidata-entity-format]. Use a buffered reader to handle I/O
  efficiently.

- **Entity Filtering:** For each line, quickly check if the line’s entity ID
  (QID) is in the set of target QIDs from OSM. This can be done by scanning the
  JSON for the pattern `"\"id\":\"Q...\"` early, or by doing a lightweight
  parse to get the `"id"` field. We can use a fast approach like looking for
  the first quote after `"id":` to extract the QID string without full
  deserialization. Only if the QID matches our set do we proceed to full
  parsing.

- **Deserialization:** Parse the JSON of the matched entity into a Rust struct
  or intermediate JSON value. Using `wikidata` crate’s structures, we could
  deserialize into `wikidata::Entity` which gives us claims and other fields.
  Alternatively, we parse manually using `simd-json` DOM to extract just the
  needed parts (claims of specific PIDs, etc.), which might be faster and use
  less memory than creating the entire object graph.

- **Extract Claims:** From the parsed data, pull out the values for each
  property of interest. This will involve looking up the `claims` map in the
  entity for keys "P1435", "P31", etc. Each claim in Wikidata is an array of
  statements with potentially qualifiers and references. We will take the main
  value(s) of each claim (preferring **truthy** statements – i.e. those with
  rank "preferred" or "normal"). Use the `wikidata` crate’s `Claim` and
  `ClaimValue` types to get the data. For example, for P1435, each claim’s
  value might be an ItemId (Qxxx) indicating a heritage designation – collect
  those QIDs. For P18, the claim value will be a `CommonsMedia` (filename
  string). For dates (P1619, P571), the value will be a time object; convert it
  to our desired format (extract year or full date string). Coordinates (P625)
  come as a globe coordinate object with lat/lon; extract the numeric lat and
  lon.

- **Normalization:** Convert each extracted value to the target type (as
  discussed in the design above). E.g. any `Qid` values -> string "Qxxx" (or
  numeric), any `Time` -> `NaiveDate` or year, any `MonolingualText` or string
  -> Rust `String`, etc. Handle units for elevation: if the value is in metres
  (very likely, since elevation usually is), we can take it directly; if it
  were another unit, we could convert to metres.

- **Insert into SQLite:** Prepare to insert the data for this entity into the
  SQLite database. We will likely have a table (say, `wikidata_claims`) with
  columns: `qid TEXT PRIMARY KEY`, `osm_id INTEGER`, `heritage_status TEXT` (or
  JSON text of array), `instance_of TEXT` (JSON array), `subclass_of TEXT`,
  `image TEXT`, `opening_date TEXT` (or INT year), `inception_date TEXT` (or
  INT), `website TEXT`, `commons_category TEXT`, `elevation INTEGER`,
  `latitude REAL`, `longitude REAL`. (The exact schema will be designed in the
  next step.) Using `rusqlite`, we execute an INSERT or REPLACE statement with
  the extracted values bound. This operation is done for each matching entity.
  We will wrap these insertions in a transaction and commit periodically (e.g.
  every N inserts or at the end) to optimize write performance.

- **Logging/Stats:** The parser can log progress, e.g. “Processed X entities, Y
  matched QIDs, inserted into DB” every few million lines, to track progress
  through the ~100M lines. After completion, it should report how many entities
  were matched and inserted, and perhaps any issues (like missing expected
  fields).

This parser needs to be robust to anomalies – e.g. if a claim is missing or in
an unexpected format, it should handle gracefully (perhaps log a warning and
skip that value rather than crash). By the end of this step, we will have built
the **Wikidata->SQLite ETL** that results in an SQLite file filled with the
desired POI claim data.

- **Design & Create SQLite Schema (pois.db):** Define the database schema to
  store the extracted claims in an indexed, queryable way. We anticipate one or
  more tables:

- One approach is a single table `wikidata_claims` keyed by `qid`. It might
  have columns for each property of interest as described above. For
  multi-valued properties, we can use a text column containing a JSON array or
  a delimiter-separated list. We will create this table and appropriate
  indexes. A PRIMARY KEY on `qid` is useful since each Wikidata item is unique;
  lookups by QID will be O(log n). If we include `osm_id`, that can either be a
  separate indexed column or even a primary key as well (if one-to-one mapping,
  we could use `osm_id` as primary key instead). We should also add an index on
  `osm_id` if it's not the primary key, to allow fast lookup by OSM POI (for
  joining when enriching the main POI data).

- If we choose to embed this into the existing `pois` table structure, an
  alternative is adding the new columns to `pois` in SQLite itself. However,
  since the `pois` table in SQLite currently stores tags as a JSON and might
  not yet have those columns, it could be cleaner to use a separate table for
  the enrichment. We can always join or update the `pois` table from it if
  needed. For now, assume a separate table as above.

- Example DDL for SQLite (to be executed via `rusqlite` before insertion
  begins):

```sql
CREATE TABLE wikidata_claims (
    qid TEXT PRIMARY KEY,
    osm_id INTEGER,
    heritage_status JSON,   -- JSON array of heritage QIDs (or NULL if none)
    instance_of   JSON,     -- JSON array of instance-of QIDs
    subclass_of   JSON,     -- JSON array of subclass-of QIDs
    image         TEXT,     -- Commons image filename
    official_open TEXT,     -- Opening date (ISO string or year)
    inception     TEXT,     -- Inception date (ISO or year)
    website       TEXT,     -- Official website URL
    commons_cat   TEXT,     -- Commons category name
    elevation     INTEGER,  -- elevation in metres
    lat           REAL,     -- latitude (if P625 present)
    lon           REAL      -- longitude (if P625 present)
);
CREATE INDEX idx_wikidata_osmid ON wikidata_claims(osm_id);
```

(The exact column types and usage of JSON can be adjusted based on how we
prefer to query; SQLite’s JSON support would allow queries like
`SELECT qid FROM wikidata_claims WHERE JSON_EXTRACT(instance_of, '$') LIKE '%Q33506%'`
 to find museums, for example.)

- The schema should be **indexed for fast lookups**, especially by QID or by
  OSM ID. The primary key or a unique index on QID will serve for QID lookups.
  If we plan to frequently go from OSM POI to Wikidata, an index on `osm_id` is
  important (not every POI has one, but those that do can be quickly found).

- If using a unified `pois.db` for both OSM and Wikidata, we may simply include
  this new table alongside the existing `pois` table. Alternatively, we might
  integrate the columns into `pois` (e.g. adding columns for each property).
  The roadmap suggests creating `pois.db` with claims, implying possibly a
  unified DB. We’ll proceed with the additional table approach for clarity. (In
  a later step, the data can be merged or queried via JOIN in SQLite or upon
  import to Postgres.)

- After designing, the code will execute the `CREATE TABLE` (and
  `CREATE INDEX`) statements via `rusqlite` before starting the insertion loop
  in the parser. This ensures the database file is set up correctly. We’ll also
  store metadata, e.g. a version number or timestamp of the dump in an `info`
  table, which can be useful for debugging (e.g. a table
  `metadata(key TEXT PRIMARY KEY, value TEXT)` with entries like
  `('wikidata_dump_date','2025-10-01')`).

With these tasks completed, the Wikidata ETL pipeline will be in place. As a
final integration step (beyond the scope of building the ETL itself), the
**Wildside CLI** `ingest` command will orchestrate the entire flow: it will
call the OSM PBF ingestion to build base POIs, then run the Wikidata ETL to
enrich those POIs, then construct the spatial index. Ultimately, we will
produce three artefacts ready for use by the engine: `pois.db` (SQLite with
enriched POI data)[^design-533-541], `pois.rstar` (the spatial R-tree index),
and `popularity.bin` (computed later, once global popularity is implemented).
This achieves a robust, efficient pipeline for semantic enrichment, aligning
with the engine’s design of using offline precomputed data for online
performance.[^design-313-320][^design-412-416] The resulting enriched POIs can
be loaded into Postgres and accessed with minimal latency, enabling rich
querying and personalization without hitting live Wikidata services.

[^design-285-293]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L285-L293>
[^design-302-308]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L302-L308>
[^qwikidata-dump-overview]: <https://qwikidata.readthedocs.io/en/stable/json_dump.html#:~:text=form%20of%20compressed%20JSON%20files,From%20the%20docs>
[^qwikidata-database-download]: <https://qwikidata.readthedocs.io/en/stable/json_dump.html#:~:text=,%E2%80%94https%3A%2F%2Fwww.wikidata.org%2Fwiki%2FWikidata%3ADatabase_download>
[^design-316-320]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L316-L320>
[^design-294-301]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L294-L301>
[^design-313-320]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L313-L320>
[^design-533-541]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L533-L541>
[^design-529-537]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L529-L537>
[^design-294-302]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L294-L302>
[^design-380-388]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L380-L388>
[^design-290-298]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L290-L298>
[^wd2sql-id-structure]: <https://github.com/p-e-w/wd2sql#:~:text=Wikidata%20IDs%20consist%20of%20a,for%20form%20and%20sense%20IDs>
[^wd2sql-see]: <https://github.com/p-e-w/wd2sql#:~:text=,see>
[^wd2sql-available]: <https://github.com/p-e-w/wd2sql#:~:text=Wikidata%20that%20is%20currently%20available>
[^wd2sql-ram]: <https://github.com/p-e-w/wd2sql#:~:text=,around%2010%20Megabytes%20of%20RAM>
[^wikidata-serialization-note]: <https://docs.rs/wikidata/latest/wikidata/#:~:text=%C2%A7A%20note%20on%20serialization>
[^wikidata-claim-value]: <https://docs.rs/wikidata/latest/wikidata/#:~:text=Claim%20Value%20%20A%20claim,Lid>
[^topicseed-hard]: <https://topicseed.com/blog/importing-wikidata-dumps/#:~:text=Unfortunately%2C%20this%20is%20the%20hard,hundreds%20of%20edits%20every%20minute>
[^topicseed-recent-changes]: <https://topicseed.com/blog/importing-wikidata-dumps/#:~:text=Functions%20every%20minute,the%20same%20conditions%20listed%20above>
[^design-412-416]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L412-L416>
[^design-404-412]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L404-L412>
[^design-312-320]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L312-L320>
[^design-413-416]: <https://github.com/leynos/wildside-engine/blob/134b542b18b85a37d1ed0f21509d8922c5d2a9b6/docs/wildside-engine-design.md#L413-L416>
[^wikidata-license]: <https://docs.rs/wikidata/latest/wikidata/#:~:text=%2A%20Apache>
[^qwikidata-entity-format]: <https://qwikidata.readthedocs.io/en/stable/json_dump.html#:~:text=represented.%20,%E2%80%9D>
