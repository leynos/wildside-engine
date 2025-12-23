Feature: Golden routes regression

  Golden routes are small, well-defined problem instances with known solutions.
  These scenarios verify that the VRP solver produces consistent, correct results
  across code changes, acting as regression tests for the solver's behaviour.

  Scenario: Single POI is always visited
    Given a golden route "trivial_single_poi"
    When the VRP solver solves the golden route
    Then the route contains the expected POIs
    And the score is within expected range

  Scenario: Budget constraint prevents visiting all POIs
    Given a golden route "budget_constrained"
    When the VRP solver solves the golden route
    Then the route respects the time budget

  Scenario: Empty candidates yield empty route
    Given a golden route "empty_candidates"
    When the VRP solver solves the golden route
    Then an empty route with zero score is returned
