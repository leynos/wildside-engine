Feature: Download the latest Wikidata dump
  Scenario: downloading the latest dump descriptor
    Given a dump status manifest containing a JSON dump
    And a writable output directory
    And a download log target
    When I download the latest dump
    Then the archive is written to disk
    And the download log records an entry

  Scenario: reporting a missing dump
    Given a dump status manifest missing the JSON dump
    And a writable output directory
    When I download the latest dump
    Then an error about the missing dump is returned
