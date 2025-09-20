# Scenario order is validated by scenario_indices_follow_feature_order in
# osm_ingest_behaviour.rs.
# Keep this ordering in sync with the scenario indices consumed by rstest-bdd.
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
