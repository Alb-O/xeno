{
  pname = "xeno";
  build.postInstall = ''
    wrapProgram $out/bin/xeno \
      --set XENO_BROKER_BIN $out/bin/xeno-broker
  '';
}
