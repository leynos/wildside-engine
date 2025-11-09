Feature: configuring the ingest command

  Scenario: selecting dataset paths via CLI flags
    Given dataset files exist on disk
    And I pass the dataset file paths with CLI flags
    When I configure the ingest command
    Then the ingest plan uses the CLI-provided dataset paths

  Scenario: rejecting missing arguments
    Given dataset files exist on disk
    And I omit all dataset configuration
    When I configure the ingest command
    Then the CLI reports that the "osm-pbf" flag is missing
