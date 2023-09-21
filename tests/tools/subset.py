# subset.py
# 
# This utility is designed to generate a subset of a test suite.
# Given a percentage, source and destination paths, all JSON and JSON.gz files
# in the source directory will be parsed and the specified percentage of tests 
# exported to the same filename in the destination directory.
#
# For example, if you have a test suite that contains 10,000 tests for each 
# opcode, issuing:
#
# subset.py 10 D:\bigtests D:\smalltests
#
# will generate a test suite in D:\smalltests containing 1,000 tests for each 
# opcode.
#
# Rationale: It may be desirable to operate on a smaller test suite for speed,
# for example, to validate tests on a commit hook or other CI task.

import json
import os
import gzip
import sys
import shutil

def get_files_in_directory(path):
    """Get all the JSON and gzipped JSON files in the given directory."""
    for filename in os.listdir(path):
        if filename.endswith('.json') or filename.endswith('.json.gz'):
            yield os.path.join(path, filename)

def load_json_file(filename):
    """Load JSON data from the given filename."""
    if filename.endswith('.json'):
        with open(filename, 'r') as f:
            return json.load(f)
    elif filename.endswith('.json.gz'):
        with gzip.open(filename, 'rt') as f:
            return json.load(f)

def save_json_file(filename, data):
    """Save JSON data to the given filename."""
    if filename.endswith('.json'):
        with open(filename, 'w') as f:
            json.dump(data, f)
    elif filename.endswith('.json.gz'):
        with gzip.open(filename, 'wt') as f:
            json.dump(data, f)

def filter_percentage(data, percentage):
    """Return the first N items based on the specified percentage."""
    num_items = int(len(data) * percentage / 100)
    return data[:num_items]

def main():
    if len(sys.argv) != 4:
        print("Usage: subset.py <percentage> <source_path> <destination_path>")
        sys.exit(1)

    percentage = float(sys.argv[1])
    source_path = sys.argv[2]
    destination_path = sys.argv[3]

    if not os.path.exists(destination_path):
        os.makedirs(destination_path)

    for source_file in get_files_in_directory(source_path):
        data = load_json_file(source_file)

        # Check if data is a list before filtering
        if not isinstance(data, list):
            print(f"Copying file {source_file} unchanged as it does not contain a JSON array.")
            shutil.copy(source_file, os.path.join(destination_path, os.path.basename(source_file)))
            continue

        filtered_data = filter_percentage(data, percentage)
        dest_file = os.path.join(destination_path, os.path.basename(source_file))
        save_json_file(dest_file, filtered_data)

if __name__ == "__main__":
    main()





