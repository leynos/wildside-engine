Feature: Feature flags

  Scenario: Ingest requires the store-sqlite feature
    Given valid ingest inputs exist
    When I run the ingest command
    Then the command fails because store-sqlite is disabled
