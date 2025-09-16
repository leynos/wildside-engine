Feature: Solver validation

  Background:
    Given a dummy solver

  Scenario: Valid request returns a response
    Given a valid solve request
    When I run the solver
    Then a successful response is produced

  Scenario: Zero duration request fails
    Given a solve request with zero duration
    When I run the solver
    Then an invalid request error is returned

  Scenario: Non-finite start request fails
    Given a solve request with a non-finite start coordinate
    When I run the solver
    Then an invalid request error is returned
