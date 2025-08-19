Feature: Interest profiles

  Scenario: known theme
    Given an interest profile with history weight 0.8
    When I query the weight for history
    Then I get 0.8

  Scenario: unknown theme
    Given an interest profile with history weight 0.8
    When I query the weight for art
    Then no weight is returned
