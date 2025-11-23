Feature: running the ingest pipeline end-to-end

  Scenario: building artefacts from valid inputs
    Given a valid OSM fixture and Wikidata dump
    When I run the ingest pipeline
    Then the pois.db and pois.rstar artefacts are created
    And the spatial index matches the ingested POI count

  Scenario: failing when the Wikidata dump is missing
    Given a valid OSM fixture and a missing Wikidata dump
    When I run the ingest pipeline
    Then the CLI reports a missing Wikidata dump
