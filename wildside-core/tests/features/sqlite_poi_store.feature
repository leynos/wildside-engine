Feature: SqlitePoiStore spatial queries

  Background:
    Given a temporary directory for SQLite artefacts

  Scenario: POI returned inside bbox
    Given a SQLite POI dataset containing a point at the origin
    When I open the SQLite POI store
    And I query the bbox covering the origin
    Then one POI is returned from the SQLite store

  Scenario: Empty when outside bbox
    Given a SQLite POI dataset containing a point at the origin
    When I open the SQLite POI store
    And I query the bbox that excludes the origin
    Then no POIs are returned from the SQLite store

  Scenario: Opening fails when index references missing POI
    Given a SQLite dataset whose index references a missing POI
    When I open the SQLite POI store
    Then opening the SQLite store fails with a missing POI error
