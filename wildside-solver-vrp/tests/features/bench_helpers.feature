Feature: Benchmark helper functions

  Scenario: Generating POIs produces a clustered distribution
    Given a request for 50 POIs with seed 42
    When POIs are generated
    Then 50 POIs are returned
    And each POI has a valid ID
    And each POI has a theme tag

  Scenario: Generating travel time matrix produces valid routing data
    Given a set of 10 POIs at known locations
    When a travel time matrix is generated
    Then the matrix is square
    And diagonal entries are zero
    And off-diagonal entries are positive

  Scenario: Same seed produces identical POIs
    Given a request for 20 POIs with seed 100
    When POIs are generated twice with the same seed
    Then both sets of POIs are identical

  Scenario: Same seed produces identical travel time matrix
    Given a set of 5 POIs at known locations
    When a travel time matrix is generated twice with the same seed
    Then both matrices are identical
