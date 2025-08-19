Feature: domain type validation

  Scenario: create point of interest with tag
    When I create a point of interest with id 1 and tag name=Park
    Then the point of interest is created

  Scenario: fail to create point of interest without tags
    When I create a point of interest with id 1 and no tags
    Then a point of interest error is returned

  Scenario: create interest profile with valid weight
    When I create an interest profile with theme art and weight 0.5
    Then the interest profile is created

  Scenario: fail to create interest profile with invalid weight
    When I create an interest profile with theme art and weight 1.5
    Then an interest profile error is returned

  Scenario: create route with point
    Given a point of interest with id 1 and tag name=Museum
    When I create a route with that point and duration 10
    Then the route is created

  Scenario: fail to create route with no points
    When I create a route with no points and duration 10
    Then a route error is returned
