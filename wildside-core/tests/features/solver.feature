Feature: Solver

  Background:
    Given a dummy solver

  Scenario: Solve succeeds with positive duration
    When I solve with duration 10 minutes
    Then a solve response is returned

  Scenario: Solve fails with zero duration
    When I solve with duration 0 minutes
    Then a solve error is returned
