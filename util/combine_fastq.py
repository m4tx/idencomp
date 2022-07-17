#!/usr/bin/env python3

filename = 'data/SRR1518133_1.fastq'

START = 0
END = 500000

f = open(filename, 'r')
cum_acids = ''
cum_q_scores = ''

while len(cum_acids) < END:
    title = f.readline().strip()
    acids = f.readline().strip()
    separator = f.readline().strip()
    q_scores = f.readline().strip()

    cum_acids += acids
    cum_q_scores += q_scores

print(f'@{filename} {START}:{END}')
print(cum_acids[START:END])
print(f'+')
print(cum_q_scores[START:END])
