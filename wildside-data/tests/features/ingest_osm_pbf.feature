# Scenario order is validated by scenario_indices_follow_feature_order in
# osm_ingest_behaviour.rs. rstest-bdd v0.1.0-alpha1 only exposes index-based
# bindings, so keep this order stable when editing scenarios.
Feature: ingesting OSM PBF data

  Scenario: summarising a known dataset
    Given a valid PBF file containing 3 nodes, 1 way and 1 relation
    When I ingest the PBF file
    Then the summary includes 3 nodes, 1 way and 1 relation
    And the summary bounding box spans the sample coordinates

  Scenario: reporting a missing file
    Given a path to a missing PBF file
    When I ingest the PBF file
    Then an open error is returned

  Scenario: rejecting a corrupted dataset
    Given a path to a file containing invalid PBF data
    When I ingest the PBF file
    Then a decode error is returned

  Scenario: extracting points of interest from tagged data
    Given a PBF file containing tourism and historic features
    When I ingest the PBF file
    Then the summary includes 3 nodes, 3 ways and 1 relation
    And the report lists 3 points of interest
    And the POI named "Museum Island Walk" uses the first node location
    And POIs referencing missing nodes are skipped

  Scenario: ignoring irrelevant tags
    Given a PBF file containing only irrelevant tags
    When I ingest the PBF file
    Then no points of interest are reported

