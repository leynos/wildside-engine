Feature: Compute global popularity

  Scenario: Heritage sites rank highest when sitelink counts exist
    Given a SQLite POI database with sitelink counts
    When I compute popularity scores
    Then the heritage POI has the highest normalised score

  Scenario: Invalid sitelink tags stop scoring
    Given a SQLite POI database with malformed sitelinks
    When I compute popularity scores
    Then popularity computation fails because sitelinks are invalid
