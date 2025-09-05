Feature: Scorer trait

  Scenario: Matching tag returns weight
    Given a POI tagged 'art' and a profile with 'art' weight 0.7
    When I score the POI
    Then the score is 0.7

  Scenario: Non-matching tag yields zero
    Given a POI tagged 'history' and a profile with 'art' weight 0.7
    When I score the POI
    Then the score is 0.0
