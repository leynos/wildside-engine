Feature: Travel time provider

  Scenario: Matrix returned for POIs
    Given a provider returning unit distances
    When I request travel times for two POIs
    Then a 2x2 matrix is returned

  Scenario: Error on empty input
    Given a provider returning unit distances
    When I request travel times for no POIs
    Then an error is returned
