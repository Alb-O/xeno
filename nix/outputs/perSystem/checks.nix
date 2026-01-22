{
  self',
  ...
}:
{
  # Package build implicitly runs tests via doCheck
  build = self'.packages.rust;
}
