Feature: VRP solver

  Scenario: Solving a valid request returns a route
    Given a memory POI store with points near the origin
    And a unit travel time provider
    And a tag scorer
    And a valid solve request with interests
    When the VRP solver runs
    Then a route is returned containing in-bbox POIs
    And the route score is positive

  Scenario: Travel time failures are surfaced as invalid request
    Given a memory POI store with points near the origin
    And a failing travel time provider
    And a tag scorer
    And a valid solve request with interests
    When the VRP solver runs
    Then the solve fails with InvalidRequest

  Scenario: No candidates yields empty route
    Given a memory POI store with no points near the origin
    And a unit travel time provider
    And a tag scorer
    And a valid solve request with interests
    When the VRP solver runs
    Then an empty route is returned

