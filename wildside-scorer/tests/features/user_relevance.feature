Feature: User relevance scoring blends popularity with interests

  Scenario: Matching interests boost the score
    Given a SQLite POI database with themed claims
    And a popularity file where the POI scores 0.3
    When I score the POI for an art-loving visitor
    Then the score combines popularity with the art interest

  Scenario: Unmatched interests fall back to popularity
    Given a SQLite POI database with themed claims
    And a popularity file where the POI scores 0.7
    When I score the POI for a food-loving visitor
    Then the score equals the popularity component

  Scenario: Missing popularity relies on the interest match
    Given a SQLite POI database with themed claims
    And a popularity file without an entry for the POI
    When I score the POI for a history-loving visitor
    Then the score is driven by the history interest
