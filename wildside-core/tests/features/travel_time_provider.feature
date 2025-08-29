Feature: Travel time provider

  Background:
    Given a provider returning unit travel times

  Scenario: Matrix returned for POIs
    When I request travel times for two POIs
    Then a 2x2 matrix is returned

  Scenario: Error on empty input
    When I request travel times for no POIs
    Then an error is returned

  Scenario: Single POI returns zero duration
    When I request travel times for one POI
    Then a 1x1 zero matrix is returned

  Scenario: Three POIs return symmetric matrix
    When I request travel times for three POIs
    Then a 3x3 symmetric unit matrix is returned

