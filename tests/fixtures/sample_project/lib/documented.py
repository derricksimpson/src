class DocumentedClass:
    """A well-documented Python class.
    Handles document processing.
    """
    def __init__(self, name):
        self.name = name

    def process(self):
        """Process the document."""
        pass

def standalone_function(x):
    """Compute the result for x."""
    return x * 2

def no_docstring():
    return True
