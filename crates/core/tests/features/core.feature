Feature: Core

    Scenario: testing test runner
        Given module at "test.js"
        When calling function "testConsole"
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
