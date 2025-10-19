Feature: Persist Wikidata claims to SQLite

  Scenario: Persist UNESCO heritage claims for a linked POI
    Given a SQLite POI database containing Berlin
    And extracted heritage claims for Berlin
    When I persist the Wikidata claims
    Then the UNESCO heritage designation is stored for that POI

  Scenario: Reject persistence when the POI is missing
    Given a SQLite POI database without the linked entity
    And extracted heritage claims for Berlin
    When I persist the Wikidata claims
    Then persistence fails because the POI is missing
