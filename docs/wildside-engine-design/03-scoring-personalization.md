# Section 2: Implementing the POI Scoring and Personalization Algorithm

This section translates the abstract scoring formula,
Score(POI)=wp​⋅P(POI)+wu​⋅U(POI,user_profile), into a concrete implementation
plan. This logic will be encapsulated within a dedicated `Scorer` component,
leveraging the data artefacts produced by the pipeline in Section 1.

## 2.1. Calculating Global Popularity `P(POI)`

The global popularity score, `P(POI)`, serves as a proxy for a POI's general,
objective importance. As this score is static and user-independent, it should
be computed for all relevant POIs during the offline data ingestion phase. The
output will be a compact binary file (e.g., `popularity.bin`), essentially an
array of `f32` scores keyed by an internal POI ID, which can be loaded
efficiently at runtime.

The implementation steps within the ETL pipeline are as follows:

1. After the Wikidata dump has been parsed and loaded into the local SQLite
   database, a process will iterate through each POI that has a `wikidata=*`
   tag.

2. For each POI, a series of queries will be executed against the local
   database to gather popularity metrics:

   - **Sitelink Count:** A query to count the number of sitelinks (links to
     Wikipedia articles in different languages).

   - **UNESCO World Heritage Status:** A check for the existence of a claim
     with property `P1435` (heritage designation) and value `Q9259` (UNESCO
     World Heritage Site).

3. These individual metrics are then normalized and combined using a weighted
   formula to produce a single floating-point `global_popularity_score`, which
   is then saved to the `popularity.bin` artefact.

The implemented scorer lives in the `wildside-scorer` crate. It resolves
sitelink counts from an optional `wikidata_entity_sitelinks` table, falling
back to `sitelinks` or `sitelink_count` tag entries and defaulting to zero when
no data exists. UNESCO heritage designations add a `25.0` bonus on top of the
`1.0` sitelink weight, and raw values are normalized against the run maximum
before serialization. The resulting `HashMap<u64, f32>` is persisted to
`popularity.bin` using `bincode`, providing a deterministic artefact for
request-time scoring.

## 2.2. Calculating User Relevance `U(POI, user_profile)`

The user relevance score, `U(POI, user\_profile)`, is where true
personalization occurs. This score is dynamic and must be calculated at request
time for the subset of candidate POIs retrieved from the R\*-tree spatial index.

The implementation steps at request time are as follows:

1. The application will contain a predefined, configurable mapping from
   high-level "Interest Themes" (e.g., "Modern Architecture," "Street Art") to
   specific Wikidata property-value pairs.

2. After retrieving the candidate POIs for the user's location from the
   R\*-tree, the system iterates through each one.

3. For each candidate `PointOfInterest`, the scorer performs a series of fast
   lookups against the local Wikidata database (e.g., `pois.db`) based on the
   user's active themes. For each theme that matches, a corresponding weight is
   added to the POI's temporary `user_relevance_score`.

4. Finally, the total `Score(POI)` for that request is calculated by combining
   the pre-computed `P(POI)` (loaded from `popularity.bin`) and the
   just-in-time `U(POI)` using the specified weights: wp​ and wu​.

The architectural decision to use offline, read-only data artefacts is the key
technical enabler for this entire personalization feature. Performing thousands
of property checks as indexed queries against a local database can be
accomplished in milliseconds, ensuring a responsive user experience.

**Implementation notes (Dec 2025):**

- The runtime scorer (`UserRelevanceScorer`) loads `popularity.bin` alongside a
  read-only `pois.db` connection. It queries the indexed `poi_wikidata_claims`
  view with prepared statements to keep per-POI lookups fast and predictable.
- Theme matching is declarative. A `ThemeClaimMapping` maps each `Theme` to
  one or more Wikidata `(property_id, value_entity_id)` pairs. The default
  mapping treats `Theme::History` as a proxy for UNESCO heritage status
  (`P1435 = Q9259`), with additional themes added by callers as the ETL
  surfaces richer claims.
- Per-request relevance sums the profile weights for matching themes and
  clamps the result to `0.0..=1.0`. Combining popularity and relevance uses a
  weighted mean (default 50/50). The user weight is only applied when at least
  one theme matches, so POIs without profile matches are not penalized.
