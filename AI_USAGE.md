# AI Usage

This project uses AI-assisted development and content generation tools.

## Scope of AI Usage

- **Code & Logic:** Human-authored and reviewed. AI was used for debugging assistance, syntax reference and generating boilerplate code. Spec Driven development was used to drive significant amounts of code when needed.
- **Documentation:** All base text was written by me in markdown or text files. AI assisted in prettifying and formatting the content.
- **Accuracy:** While I review all AI-assisted outputs, probabilistic errors or hallucinations may occasionally remain but I have done my best to rectify them. Please reach out or raise an issue if you find something that slipped by.

## Decisions takent against or independently

1. Deciding that API key will be split - Key ID and Key Secret similar to AWS. AI recommended stripe style single key but I have seen the the split pattern in razorpay and AWS. This also helps in finding the key quickly in DB. Stripe relies on embedding unique business key info in the api key to lookup fast, its easier to split for my scale. Confirmed my hypothesis with AI after this.
2. Assumed that PSP could be made more complex and have idempotency even though I specified we only control a mock
3. Steered AI to move on from simple pessimistic locking as tok_timeout would cause problems in the case.

## Things that AI got wrong

1. Recommending Authorization header as Bearer in first try than Basic. Using basic with key_id and key secret is better.

## Model Wise Details

### ChatGPT

1. Spec formatting and augmentation with sections like models, must haves.
2. Exploring various options I have for API Key generation and storage options.
3. Executing coding of spec design.md via kilo code
4. generating tests and test.ps1 script
5. Formatting tables in long form for data models
6. options for payment states - should unknown be there or not
7. consideration of Money as struct/trait instead of plain cents
8. Enum creation syntax
9. State machine ascii diagram

### Claude Web

1. Spec formatting for topology, do not build, mock PSP
2. Exploring various options for API key lifecycle. Validating that key_id and key_secret split has wins.
3. Comparing and contrasting different locks, why advisory lock is not useful for payment lock but for webhook retry instead
4. options to not hold lock on invoice when tok_timeout occurs
5. mock psp implementation
6. Index suggestions if anything i missed

### Open Weights

1. Coding execution of spec and design.md

## Responsibility

The human author(s) remain solely responsible for the content, accuracy, integrity, and fitness-for-purpose of this project.
