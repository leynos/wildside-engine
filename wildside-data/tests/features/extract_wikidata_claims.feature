Feature: Extract linked Wikidata claims

  Scenario: Extract UNESCO heritage designation
    Given an OSM ingest report containing linked POIs
    And a dump containing a heritage claim for the linked entity
    When I extract the linked claims
    Then the UNESCO heritage designation is recorded

  Scenario: Report malformed entities
    Given an OSM ingest report containing linked POIs
    And a dump with malformed JSON for the linked entity
    When I extract the linked claims
    Then a parse error is reported
