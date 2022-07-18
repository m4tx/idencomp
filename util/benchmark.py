#!/usr/bin/env python3

import argparse
import csv
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import List

parser = argparse.ArgumentParser(
    description='Benchmark given FASTQ file against predefined compressors '
                'and output the statistics as csv to stdout')
parser.add_argument(
    'input', help='input file path')
parser.add_argument(
    '--remove', action='store_true', help='remove the intermediate files')
args = parser.parse_args()


def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)


@dataclass
class Command:
    args: List[str]
    input_option: str = ''
    output_option: str = ''
    output_stdout: bool = False

    def command(self, input_path, output_path):
        args = self.args.copy()
        if self.input_option:
            args.append(self.input_option)
        args.append(input_path)
        if not self.output_stdout:
            if self.output_option:
                args.append(self.output_option)
            args.append(output_path)

        return args

    def run(self, input_path, output_path):
        cmd = self.command(input_path, output_path)
        if self.output_stdout:
            output_file = open(output_path, 'w')
            subprocess.run(cmd, stdout=output_file, stderr=subprocess.DEVNULL,
                           check=True)
        else:
            subprocess.run(cmd, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, check=True)

    def description(self):
        cmd = ' '.join(self.command('$INPUT', '$OUTPUT'))
        if self.output_stdout:
            cmd += f' > $OUTPUT'
        return cmd


@dataclass
class Compressor:
    name: str
    compress: Command
    decompress: Command


def human_bytes(num: float) -> str:
    suffix = ''
    if num >= 1_000_000_000:
        num /= 1_000_000_000
        suffix = 'G'
    elif num >= 1_000_000:
        num /= 1_000_000
        suffix = 'M'
    elif num >= 1_000:
        num /= 1_000
        suffix = 'k'
    return f'{num:.2f}{suffix}'


csv_writer = csv.writer(sys.stdout)
csv_writer.writerow(
    ['cmd', 'input_size', 'output_size', 'compress_time', 'decompress_time',
     'ratio', 'compress_speed', 'decompress_speed'])


def output_stat(name: str, compress_time: float, decompress_time: float,
                input_size: int, output_size: int):
    ratio = output_size / input_size
    compress_speed = input_size / compress_time
    decompress_speed = input_size / decompress_time
    eprint(f'{name:>15}: {input_size:>9} -> {output_size:>9} in '
           f'{compress_time:>7.2f}s / {decompress_time:>7.2f}s '
           f'({ratio * 100:>6.2f}%, '
           f'{human_bytes(compress_speed):>6}B/s / '
           f'{human_bytes(decompress_speed):>6}B/s)')
    csv_writer.writerow(
        [name, input_size, output_size, compress_time, decompress_time,
         ratio, compress_speed, decompress_speed])


compressors = [
    Compressor(
        'gzip',
        Command(['gzip', '-c'], output_stdout=True),
        Command(['gzip', '-c', '-d'], output_stdout=True),
    ),
    Compressor(
        'gzip_9',
        Command(['gzip', '-c', '-9'], output_stdout=True),
        Command(['gzip', '-c', '-d'], output_stdout=True),
    ),
    Compressor(
        'bzip2',
        Command(['bzip2', '-c'], output_stdout=True),
        Command(['bzip2', '-c', '-d'], output_stdout=True),
    ),
    Compressor(
        'bzip2_9',
        Command(['bzip2', '-c', '-9'], output_stdout=True),
        Command(['bzip2', '-c', '-d'], output_stdout=True),
    ),
    Compressor(
        'lzma',
        Command(['lzma', '-c', '-T', '12'], output_stdout=True),
        Command(['lzma', '-c', '-d', '-T', '12'], output_stdout=True),
    ),
    Compressor(
        'fqzcomp_q2',
        Command(['fqzcomp', '-q2', '-s5+']),
        Command(['fqzcomp', '-d']),
    ),
    Compressor(
        'fqzcomp_q3',
        Command(['fqzcomp', '-q3', '-s5+']),
        Command(['fqzcomp', '-d']),
    ),
    Compressor(
        'genozip',
        Command(['genozip'], output_option='-o'),
        Command(['genounzip'], output_option='-o'),
    ),
    Compressor(
        'spring',
        Command(['spring', '-c', '--no-ids', '-t12'],
                input_option='-i', output_option='-o'),
        Command(['spring', '-d', '-t12'],
                input_option='-i', output_option='-o'),
    ),
    Compressor(
        'dsrc2',
        Command(['dsrc', 'c', '-m1', '-t12']),
        Command(['dsrc', 'd', '-t12']),
    ),
]

input_path = Path(args.input)
input_size = input_path.stat().st_size

for compressor in compressors:
    compressed_path = Path(f'out/compressed.{compressor.name}')
    decompressed_path = Path(f'out/decompressed.{compressor.name}')

    # Compress
    start = time.perf_counter()
    compressor.compress.run(input_path, compressed_path)
    end = time.perf_counter()
    output_size = compressed_path.stat().st_size
    compress_elapsed = end - start

    # Decompress
    start = time.perf_counter()
    compressor.decompress.run(compressed_path, decompressed_path)
    end = time.perf_counter()
    decompress_elapsed = end - start

    # Stats
    output_stat(compressor.name, compress_elapsed, decompress_elapsed,
                input_size, output_size)

    # Remove the files
    if args.remove:
        compressed_path.unlink()
        decompressed_path.unlink()
