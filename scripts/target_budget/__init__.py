"""Bound one cargo target directory against what cargo says it still uses.

Import the submodules; this package deliberately re-exports nothing. `survey`
and `marks` name both a module and its main function, and binding the function
at package level shadows the module for every later importer.
"""
