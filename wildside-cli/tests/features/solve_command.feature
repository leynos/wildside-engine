Feature: solving an orienteering request

  Scenario: solving a request from JSON
    Given default solver artefacts exist on disk
    And a valid solve request exists on disk
    When I run the solve command
    Then the command succeeds and prints JSON output

  Scenario: rejecting invalid JSON input
    Given default solver artefacts exist on disk
    And the solve request contains invalid JSON
    When I run the solve command
    Then the command fails because the request JSON is invalid

  Scenario: rejecting invalid solve requests
    Given default solver artefacts exist on disk
    And the solve request contains invalid parameters
    When I run the solve command
    Then the command fails because the request is invalid

  Scenario: rejecting missing request paths
    Given default solver artefacts exist on disk
    And I omit the solve request path
    When I run the solve command
    Then the command fails because the request path is missing
