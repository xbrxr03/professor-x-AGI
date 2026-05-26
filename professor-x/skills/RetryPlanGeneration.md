# RetryPlanGeneration  
Purpose: Analyze failed tool observations and generate a bounded retry plan with parameter adjustments and safety constraints.  
Workflow:  
1. Parse the tool's observation for error type (e.g., timeout, invalid output, partial success)  
2. Identify the specific layer/lever failing (e.g., DHE:layer=3, lever=3)  
3. Generate a retry plan with:  
   - Maximum retry count (3-5)  
   - Parameter adjustments (e.g., timeout=60s → 120s)  
   - Safety constraints (e.g., code validation checks)  
4. Output the retry plan with execution steps  
Output Contract:  
- retry_count: Integer (3-5)  
- parameter_adjustments: Dictionary {key: new_value}  
- safety_constraints: List of validation rules  
- execution_steps: List of ordered operations
