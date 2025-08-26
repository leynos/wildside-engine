Feature: PoiStore bounding box queries

  # Notes:
  # - Coordinates are in a Cartesian plane for tests.
  # - Containment is boundary-inclusive (points on the bbox edge count as contained).
  # - Units are abstract; values are dimensionless test fixtures.

  Scenario: POI returned
    Given a store containing a single POI at the origin
    When I query the bbox covering the origin
    Then one POI is returned

  Scenario: Empty when outside bbox
    Given a store containing a single POI at the origin
    When I query the bbox that excludes the origin
    Then no POIs are returned

  Scenario: POI returned when on bbox boundary
    Given a store containing a single POI at the origin
    When I query the bbox whose edge passes through the origin
    Then one POI is returned
