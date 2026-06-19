import sys
from mailer import render
try:
    assert render('Hi {name}, bye {name}','Sam')=='Hi Sam, bye Sam'
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
