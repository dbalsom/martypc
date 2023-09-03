import json
import sys

def load_json(file_path):
    with open(file_path, 'r') as f:
        return json.load(f)

def save_json(data, file_path):
    with open(file_path, 'w') as f:
        json.dump(data, f, indent=4)

def concatenate_and_trim(array1, array2, size):
    concatenated = array1 + array2
    return concatenated[:size]

if __name__ == '__main__':
    if len(sys.argv) != 4:
        print("Usage: python join.py <path_to_first_json> <path_to_second_json> <size>")
        sys.exit(1)
    
    path1, path2, size = sys.argv[1], sys.argv[2], int(sys.argv[3])

    array1 = load_json(path1)
    array2 = load_json(path2)

    if not (isinstance(array1, list) and isinstance(array2, list)):
        print("Both JSON files should contain arrays.")
        sys.exit(1)

    result = concatenate_and_trim(array1, array2, size)
    save_json(result, 'out.json')

    print("Output written to 'out.json'")