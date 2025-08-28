"""
Configuration matrix generator for DSL-RS test runner.
Generates test configurations from ranges and combinations.
"""

import itertools
import yaml
import json
from typing import Dict, List, Any, Union, Optional
from pathlib import Path


class ConfigGenerator:
    """Generate test configuration matrices from specifications."""
    
    def __init__(self, config_file: Optional[Path] = None):
        """
        Initialize the configuration generator.
        
        Args:
            config_file: Optional path to YAML/JSON configuration file
        """
        self.base_config = {}
        if config_file and config_file.exists():
            self.load_config(config_file)
    
    def load_config(self, config_file: Path):
        """Load configuration from YAML or JSON file."""
        with open(config_file, 'r') as f:
            if config_file.suffix == '.yaml' or config_file.suffix == '.yml':
                self.base_config = yaml.safe_load(f)
            elif config_file.suffix == '.json':
                self.base_config = json.load(f)
            else:
                raise ValueError(f"Unsupported config file format: {config_file.suffix}")
    
    def generate_range(self, spec: Union[Dict, List]) -> List[Any]:
        """
        Generate values from a range specification.
        
        Args:
            spec: Either a dict with 'start', 'stop', 'step' or a list of values
        
        Returns:
            List of generated values
        """
        if isinstance(spec, list):
            return spec
        elif isinstance(spec, dict):
            if 'values' in spec:
                return spec['values']
            elif 'start' in spec and 'stop' in spec:
                start = spec['start']
                stop = spec['stop']
                step = spec.get('step', 1)
                
                if isinstance(start, int) and isinstance(stop, int):
                    return list(range(start, stop + 1, step))
                elif isinstance(start, float) or isinstance(stop, float):
                    values = []
                    current = float(start)
                    while current <= stop:
                        values.append(current)
                        current += step
                    return values
            elif 'enum' in spec:
                return spec['enum']
        return [spec]
    
    def generate_matrix(self, params: Dict[str, Any]) -> List[Dict[str, Any]]:
        """
        Generate configuration matrix from parameter specifications.
        
        Args:
            params: Dictionary of parameter names to range specifications
        
        Returns:
            List of configuration dictionaries
        """
        # Generate all possible values for each parameter
        param_values = {}
        for param, spec in params.items():
            param_values[param] = self.generate_range(spec)
        
        # Calculate total combinations
        total = 1
        for values in param_values.values():
            total *= len(values)
        
        print(f"Generating {total} configuration combinations")
        
        # Generate all combinations
        if not param_values:
            return [{}]
        
        keys = list(param_values.keys())
        values = [param_values[k] for k in keys]
        
        configurations = []
        for combination in itertools.product(*values):
            config = dict(zip(keys, combination))
            configurations.append(config)
        
        return configurations
    
    def generate_env_matrix(self, env_specs: Dict[str, Any]) -> List[Dict[str, str]]:
        """
        Generate environment variable configurations.
        
        Args:
            env_specs: Dictionary of environment variable specifications
        
        Returns:
            List of environment variable dictionaries
        """
        configs = self.generate_matrix(env_specs)
        # Convert all values to strings for environment variables
        env_configs = []
        for config in configs:
            env_config = {k: str(v) for k, v in config.items()}
            env_configs.append(env_config)
        return env_configs
    
    def filter_matrix(self, 
                     configurations: List[Dict[str, Any]], 
                     filter_func: Optional[callable] = None,
                     constraints: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """
        Filter configuration matrix based on constraints.
        
        Args:
            configurations: List of configurations to filter
            filter_func: Optional callable to filter configurations
            constraints: Optional dictionary of constraints
        
        Returns:
            Filtered list of configurations
        """
        filtered = configurations
        
        if filter_func:
            filtered = [c for c in filtered if filter_func(c)]
        
        if constraints:
            for key, constraint in constraints.items():
                if 'min' in constraint:
                    filtered = [c for c in filtered if c.get(key, 0) >= constraint['min']]
                if 'max' in constraint:
                    filtered = [c for c in filtered if c.get(key, float('inf')) <= constraint['max']]
                if 'exclude' in constraint:
                    excluded = constraint['exclude'] if isinstance(constraint['exclude'], list) else [constraint['exclude']]
                    filtered = [c for c in filtered if c.get(key) not in excluded]
                if 'include' in constraint:
                    included = constraint['include'] if isinstance(constraint['include'], list) else [constraint['include']]
                    filtered = [c for c in filtered if c.get(key) in included]
        
        return filtered
    
    def generate_test_scenarios(self) -> Dict[str, List[Dict[str, Any]]]:
        """
        Generate test scenarios from base configuration.
        
        Returns:
            Dictionary of scenario names to configuration lists
        """
        if 'scenarios' not in self.base_config:
            return {}
        
        scenarios = {}
        for name, spec in self.base_config['scenarios'].items():
            if 'matrix' in spec:
                configs = self.generate_matrix(spec['matrix'])
                if 'filter' in spec:
                    configs = self.filter_matrix(configs, constraints=spec['filter'])
                scenarios[name] = configs
            elif 'configs' in spec:
                scenarios[name] = spec['configs']
            else:
                scenarios[name] = [spec]
        
        return scenarios
    
    def save_matrix(self, configurations: List[Dict[str, Any]], output_file: Path):
        """
        Save configuration matrix to file.
        
        Args:
            configurations: List of configurations to save
            output_file: Path to output file
        """
        output_file.parent.mkdir(parents=True, exist_ok=True)
        
        with open(output_file, 'w') as f:
            if output_file.suffix == '.yaml' or output_file.suffix == '.yml':
                yaml.dump(configurations, f, default_flow_style=False)
            else:
                json.dump(configurations, f, indent=2)
        
        print(f"Saved {len(configurations)} configurations to {output_file}")


def create_default_matrix() -> Dict[str, Any]:
    """Create a default test matrix configuration."""
    return {
        'stream_count': {'start': 1, 'stop': 10, 'step': 3},
        'watchdog_timeout': {'values': [0, 30, 60, 300]},
        'retry_attempts': {'start': 1, 'stop': 5},
        'backoff_factor': {'values': [1.0, 1.5, 2.0]},
        'enable_metrics': {'values': [True, False]},
        'enable_health_check': {'values': [True, False]},
        'buffer_size': {'enum': ['small', 'medium', 'large']},
        'recovery_strategy': {'enum': ['immediate', 'exponential', 'circuit_breaker']}
    }


def create_stress_test_matrix() -> Dict[str, Any]:
    """Create a stress test configuration matrix."""
    return {
        'stream_count': {'start': 50, 'stop': 200, 'step': 50},
        'concurrent_operations': {'start': 10, 'stop': 100, 'step': 30},
        'memory_limit_mb': {'values': [512, 1024, 2048, 4096]},
        'cpu_limit_percent': {'values': [25, 50, 75, 100]},
        'network_latency_ms': {'values': [0, 100, 500, 1000]},
        'packet_loss_percent': {'values': [0, 1, 5, 10]},
    }


def create_compatibility_matrix() -> Dict[str, Any]:
    """Create a compatibility test matrix."""
    return {
        'gstreamer_version': {'values': ['1.16', '1.18', '1.20', '1.22']},
        'codec': {'enum': ['h264', 'h265', 'vp8', 'vp9']},
        'container': {'enum': ['mp4', 'mkv', 'avi', 'webm']},
        'resolution': {'enum': ['480p', '720p', '1080p', '4k']},
        'framerate': {'values': [15, 24, 30, 60]},
    }


if __name__ == '__main__':
    # Example usage
    generator = ConfigGenerator()
    
    # Generate default matrix
    print("Default Test Matrix:")
    default_configs = generator.generate_matrix(create_default_matrix())
    print(f"Generated {len(default_configs)} configurations")
    
    # Filter to reasonable subset
    filtered = generator.filter_matrix(default_configs, constraints={
        'stream_count': {'max': 10},
        'watchdog_timeout': {'exclude': 0}
    })
    print(f"Filtered to {len(filtered)} configurations")
    
    # Generate stress test matrix
    print("\nStress Test Matrix:")
    stress_configs = generator.generate_matrix(create_stress_test_matrix())
    print(f"Generated {len(stress_configs)} stress test configurations")
    
    # Generate compatibility matrix
    print("\nCompatibility Matrix:")
    compat_configs = generator.generate_matrix(create_compatibility_matrix())
    print(f"Generated {len(compat_configs)} compatibility configurations")