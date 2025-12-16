Feature: HTTP travel time provider
  The HTTP travel time provider fetches pairwise travel times from an OSRM
  routing service and returns an n×n matrix of durations.

  Scenario: returning a travel time matrix for two POIs
    Given a routing service returning valid durations
    When I request travel times for two POIs
    Then a 2x2 matrix is returned

  Scenario: returning an error for empty input
    Given a routing service returning valid durations
    When I request travel times for no POIs
    Then an empty input error is returned

  Scenario: handling a network error
    Given a routing service that fails with a network error
    When I request travel times for two POIs
    Then a network error is returned

  Scenario: handling a timeout
    Given a routing service that times out
    When I request travel times for two POIs
    Then a timeout error is returned

  Scenario: handling a service error response
    Given a routing service returning an error response
    When I request travel times for two POIs
    Then a service error is returned

  Scenario: handling unreachable pairs
    Given a routing service returning null for unreachable pairs
    When I request travel times for two POIs
    Then a 2x2 matrix with maximum duration for nulls is returned
