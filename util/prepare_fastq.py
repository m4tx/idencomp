#!/usr/bin/env python3

import argparse
from pathlib import Path

parser = argparse.ArgumentParser(
    description='Remove identifiers from a FASTQ file '
                'and optionally divides it into chunks')
parser.add_argument(
    'input', help='input file path')
parser.add_argument(
    '--seq-length', type=int, default=0,
    help='desired length of a single sequence')
parser.add_argument(
    '--size', type=str,
    help='desired size of the output file in bytes')
parser.add_argument(
    '--chunk', type=int, default=0,
    help='index of the chunk to output')
args = parser.parse_args()

max_size = float('inf')
if args.size is not None:
    suffix = args.size[-1]
    mult = 1
    if suffix == 'k':
        mult = 1_000
    elif suffix == 'M':
        mult = 1_000_000
    elif suffix == 'G':
        mult = 1_000_000_000

    val = int(''.join([c for c in args.size if c.isdigit()]))
    max_size = val * mult

seq_len = args.seq_length
chunk_index = args.chunk

input_path = Path(args.input)
stem = input_path.stem
stem += '.noident'
if args.size is not None:
    stem += f'.{args.seq_length}.{args.size}.{args.chunk}'
output_path = input_path.with_stem(stem)

lines = []
input_file = open(input_path, 'r')
output_file = open(output_path, 'w')

print(f'Output file path: {output_path}')

current_chunk_index = 0
size = 0
acids_chunk = ''
q_scores_chunk = ''
for line in input_file:
    lines.append(line.strip())
    if len(lines) < 4:
        continue

    identifier, acids, separator, q_scores = lines
    lines = []

    acids_chunk += acids
    q_scores_chunk += q_scores
    if len(acids_chunk) < seq_len:
        continue

    sequence = ''
    sequence += '@\n'
    sequence += f'{acids_chunk}\n'
    sequence += f'+\n'
    sequence += f'{q_scores_chunk}\n'

    acids_chunk = ''
    q_scores_chunk = ''

    if current_chunk_index == chunk_index:
        output_file.write(sequence)
    size += len(sequence)

    if size >= max_size:
        if current_chunk_index == chunk_index:
            break
        else:
            current_chunk_index += 1
            size = 0
