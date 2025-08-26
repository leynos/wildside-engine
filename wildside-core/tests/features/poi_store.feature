Feature: PoiStore bounding box queries

  Scenario: POI returned
    Given a store containing a single POI at the origin
    When I query the bbox covering the origin
    Then one POI is returned

  Scenario: Empty when outside bbox
    Given a store containing a single POI at the origin
    When I query the bbox that excludes the origin
    Then no POIs are returned

  Scenario: Boundary inclusive
    Given a store containing a single POI at the origin
    When I query the bbox whose edge passes through the origin
    Then one POI is returned
