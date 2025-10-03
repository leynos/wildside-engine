Feature: build spatial index
  The spatial index accelerates geographic queries by storing POIs in an R*-tree.

  Scenario: Query returns POI inside bounding box
    Given a collection of POIs including the city centre and riverside landmarks
    When I build the spatial index
    And I query the bbox that covers the city centre landmark
    Then exactly one POI with id 1 is returned

  Scenario: Query outside bounding box returns nothing
    Given a collection of POIs including the city centre and riverside landmarks
    When I build the spatial index
    And I query the bbox that excludes all landmarks
    Then no POIs are returned

  Scenario: Building the index from an empty collection yields an empty tree
    Given an empty collection of POIs
    When I build the spatial index
    Then the spatial index is empty
