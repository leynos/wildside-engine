Feature: Compute global popularity

  Scenario: Heritage sites rank highest when sitelink counts exist
    Given a SQLite POI database with sitelink counts
    When I compute popularity scores
    Then the heritage POI has the highest normalised score

  Scenario: Invalid sitelink tags stop scoring
    Given a SQLite POI database with malformed sitelinks
    When I compute popularity scores
    Then popularity computation fails because sitelinks are invalid

  Scenario: Unlinked POIs have zero popularity
    Given a SQLite POI database with sitelink counts
    When I compute popularity scores
    Then the unlinked POI has a zero normalised score

  Scenario: Popularity file round-trips computed scores
    Given a SQLite POI database with sitelink counts
    When I write the popularity file to a nested path
    Then the popularity file round-trips the scores
