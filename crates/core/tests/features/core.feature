Feature: Core

    Scenario: testing runner
        Given module at "test.js"
        When calling function "testConsole"

    Scenario: testing return value
        Given module at "test.js"
        When calling function "testReturnValue" with args ("Runner")
        Then return value is "Hello Runner"

    Scenario: testing args
        Given module at "test.js"
        When calling function "testArgs" with args ("One", "Two", "Three", "Four", "Five")
        Then return value is "One Two Three Four Five"

    Scenario: get class name
        Given module at "test.js"
        When calling function "getClassName"
        Then return value is "Object"

    Scenario: call basic method
        Given module at "test.js"
        When calling function "callMethod" with args ("Object", "className")
        Then return value is "Ob"

    Scenario: construct class
        Given module at "test.js"
        When calling function "constructClass"
        Then return value is ()

# Then that string is now equal to "something interesting"

# Scenario: async things
#     Given some awkward async process to get a string
#     And some tables being filled with data
#     When we check a database we have prefilled with a string
#     Then we find we somehow had the same string

# Scenario Outline: eating
#     Given there are <start> cucumbers
#     When I eat <eat> cucumbers
#     Then I should have <left> cucumbers

#     Examples:
#         | start | eat | left |
#         | 12    | 5   | 7    |
#         | 20    | 5   | 15   |
