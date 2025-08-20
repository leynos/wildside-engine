Feature: Interest profiles

  Scenario Outline: Querying interest profile weights
    Given an interest profile with weights <weights>
    When I query the weight for "<theme>"
    Then the result is <expected>

    Examples:
      | weights                      | theme    | expected |
      | {"history": 0.8}             | history  | 0.8      |
      | {"history": 0.8}             | art      | null     |
      | {}                           | history  | null     |
      | {"history": 0.8, "art": 0.3} | history  | 0.8      |
      | {"history": 0.8, "art": 0.3} | art      | 0.3      |
      | {"history": 0.8}             | invalid  | error    |
