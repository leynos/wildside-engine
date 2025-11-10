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

  Scenario: layering CLI, config file, and environment values
    Given dataset files exist on disk
    And the dataset file paths are provided via a config file
    And the Wikidata path is overridden via environment variables
    And I pass only the OSM CLI flag
    When I configure the ingest command
    Then CLI and environment layers override configuration defaults
