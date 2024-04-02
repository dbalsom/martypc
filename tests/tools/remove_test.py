import json
import os
import sys

def remove_test_from_json(file_path, test_number):
    """
    Remove a specific test from a JSON file given its index (test_number).
    """
    # Ensure the file exists
    if not os.path.exists(file_path):
        print(f"Error: File {file_path} does not exist.")
        return

    # Load the JSON data from the file
    with open(file_path, 'r') as f:
        try:
            data = json.load(f)
        except json.JSONDecodeError:
            print(f"Error: File {file_path} is not a valid JSON.")
            return

    # Ensure data is a list
    if not isinstance(data, list):
        print(f"Error: File {file_path} does not contain a JSON array.")
        return

    # Ensure the test number is valid
    if test_number < 0 or test_number >= len(data):
        print(f"Error: Invalid test number {test_number}. Valid range is 0 to {len(data) - 1}.")
        return

    # Remove the test
    del data[test_number]

    # Save the modified data back to the file
    with open(file_path, 'w') as f:
        json.dump(data, f, indent=4)

    print(f"Removed test number {test_number} from {file_path}.")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: remove_test.py <path_to_json_file> <test_number>")
        sys.exit(1)

    file_path = sys.argv[1]
    try:
        test_number = int(sys.argv[2])
    except ValueError:
        print("Error: Test number must be an integer.")
        sys.exit(1)

    remove_test_from_json(file_path, test_number)