Feature: Interest profiles

  Scenario: known theme
    Given an interest profile with history weight 0.8
    When I query the weight for history
    Then I get 0.8

  Scenario: unknown theme
    Given an interest profile with history weight 0.8
    When I query the weight for art
    Then no weight is returned

  Scenario: empty profile
    Given an empty interest profile
    When I query the weight for history
    Then no weight is returned

  Scenario: multiple themes
    Given an interest profile with history weight 0.8
    And an interest profile with art weight 0.3
    When I query the weight for history
    Then I get 0.8
    When I query the weight for art
    Then I get 0.3
