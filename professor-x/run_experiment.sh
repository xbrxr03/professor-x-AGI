#!/bin/bash
# Measure performance variability
response_times=( $(./benchmark_tool --rounds 30) )
std_dev=$(echo "${response_times[@]}" | awk '{sum=0; for(i=1; i<=NF; i++) sum+=$i^2; mean=sum/NF; sum=0; for(i=1; i<=NF; i++) sum+=$(i)-mean; sum /= NF; printf "%.4f\n", sqrt(sum)}')
 echo "Performance Variability (SD): $std_dev"