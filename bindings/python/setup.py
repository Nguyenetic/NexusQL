from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="nexusql",
    version="2.3.0",
    author="John Nguyen",
    author_email="john@nexusql.io",
    description="AI-Native, PostgreSQL-compatible database with Python support",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/johnhnguyen97/NexusQL",
    packages=find_packages(),
    classifiers=[
        "Development Status :: 5 - Production/Stable",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Database",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    python_requires=">=3.8",
    install_requires=[
        # In production: would depend on native Rust module
    ],
    extras_require={
        "dev": ["pytest>=7.0.0", "black>=22.0.0", "mypy>=0.950"],
    },
    keywords="database postgresql sql embedded ai-native vector-search rust",
    project_urls={
        "Bug Reports": "https://github.com/johnhnguyen97/NexusQL/issues",
        "Source": "https://github.com/johnhnguyen97/NexusQL",
        "Documentation": "https://github.com/johnhnguyen97/NexusQL#readme",
    },
)
