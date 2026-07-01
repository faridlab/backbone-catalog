# Catalog acceptance oracle — backbone-catalog
# Flow map:    docs/business-flows/catalog.md
# Golden cases: docs/business-flows/golden-cases.md
# Executable truth: tests/catalog_golden_cases.rs + tests/integrity_probes.rs

Feature: Define the canonical product catalog
  In order to give every ERP module one stable product/service identity to project
  As a catalog admin
  I want to create item groups, units, and items under validated rules

  Background:
    Given the tenant schema "catalog" is migrated

  @happy-path @module:catalog @cgc-2
  Scenario: Create an item under an existing group and unit
    Given an item group "Finished Goods" exists
    And a unit of measure "PCS" exists
    When I create item "SKU-1" in that group with default unit "PCS"
    Then it is created with status "active" and type "physical_good"

  @validation @module:catalog @cgc-3
  Scenario: An item pointing at a missing group is rejected
    When I create an item with a non-existent item group
    Then the request is rejected with "item_group_not_found"

  @validation @module:catalog @cgc-5
  Scenario: An item with no usage is rejected
    When I create an item that is neither sellable, purchasable, nor stocked
    Then the request is rejected with "no_usage_flag"

  @validation @module:catalog @cgc-6
  Scenario: A duplicate item code is rejected
    Given an item "SKU-1" already exists
    When I create another item with code "SKU-1"
    Then the request is rejected with "duplicate_item_code"

  @validation @module:catalog @cgc-7
  Scenario: A self-referential UOM conversion is rejected
    Given a unit "KG" exists
    When I create a UOM conversion from "KG" to "KG"
    Then the request is rejected with "same_uom"

  @happy-path @module:catalog @cgc-9
  Scenario: A valid UOM conversion is stored
    Given units "BOX" and "PCS" exist
    When I create a UOM conversion of 1 "BOX" = 12 "PCS"
    Then it is created with factor 12

  @guard @module:catalog @igc-1
  Scenario: The generic item create route is not exposed on the guarded surface
    When I POST to "/items" on the guarded routes with a generic payload
    Then the response status is 405 or 404

  @variants @module:catalog @vgc-2
  Scenario: Create a variant from valid options and flip the item to has_variants
    Given a template item "TSHIRT" exists
    And an attribute "color" with value "red" (label "Red")
    And an attribute "size" with value "m" (label "M")
    When I create a variant of "TSHIRT" with sku "TSHIRT-RED-M" and options color=red, size=m
    Then the variant label is "Red / M"
    And the item "TSHIRT" now has_variants true

  @variants @module:catalog @vgc-3
  Scenario: A variant with an unregistered option value is rejected
    Given a template item "TSHIRT" exists
    And an attribute "color" with value "red"
    When I create a variant of "TSHIRT" with option color=purple
    Then the request is rejected with "unknown_attribute_value"
