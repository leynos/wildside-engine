Feature: Scorer trait

  Scenario: Matching tag returns weight
    Given a POI tagged 'art' and a profile with 'art' weight 0.7
    When I score the POI
    Then the result is 0.7

  Scenario: Non-matching tag yields zero
    Given a POI tagged 'history' and a profile with 'art' weight 0.7
    When I score the POI
    Then the result is 0.0

  Scenario: Multiple matching tags sum weights
    Given a POI tagged 'art' and 'history' and a profile with 'art' weight 0.7 and 'history' weight 0.2
    When I score the POI
    Then the result is 0.9

  Scenario: Unknown tag returns zero
    Given a POI tagged 'unknown_tag' and a profile with 'art' weight 0.7
    When I score the POI
    Then the result is 0.0

  Scenario: POI without tags returns zero
    Given a POI with no tags and a profile with 'art' weight 0.7
    When I score the POI
    Then the result is 0.0
